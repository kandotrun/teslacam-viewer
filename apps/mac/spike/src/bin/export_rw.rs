//! Q6 alternative: AVAssetReaderVideoCompositionOutput (same 6-track layer
//! transforms, no CoreAnimation tool) -> Core Image overlay (labels + vehicle,
//! pre-rendered once from the same CALayer tree) -> AVAssetWriter H.264 with an
//! explicit bitrate (AVAssetExportSession gives no bitrate control).
//!
//! Usage: export_rw <out.mp4> [bitrate_bps]
use std::{
    ffi::CString,
    path::PathBuf,
    ptr::NonNull,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use block2::RcBlock;
use objc2::{rc::Retained, runtime::{AnyObject, ProtocolObject}};
use objc2_av_foundation::{
    AVAsset, AVAssetReader, AVAssetReaderVideoCompositionOutput, AVAssetWriter,
    AVAssetWriterInput, AVAssetWriterInputPixelBufferAdaptor, AVFileTypeMPEG4, AVMediaTypeVideo,
    AVMutableComposition, AVMutableVideoComposition, AVMutableVideoCompositionInstruction,
    AVMutableVideoCompositionLayerInstruction, AVVideoAverageBitRateKey, AVVideoCodecKey,
    AVVideoCodecTypeH264, AVVideoCompositionInstructionProtocol,
    AVVideoCompositionLayerInstruction, AVVideoCompressionPropertiesKey, AVVideoHeightKey,
    AVVideoMaxKeyFrameIntervalKey, AVVideoWidthKey,
};
use objc2_core_foundation::{CFRetained, CFString, CGAffineTransform, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGColor, CGColorRenderingIntent,
    CGColorSpace, CGDataProvider, CGImage,
};
use objc2_core_image::{CIContext, CIImage};
use objc2_core_media::{CMTime, CMTimeRange, kCMTimeZero};
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferPool, kCVPixelBufferHeightKey, kCVPixelBufferIOSurfacePropertiesKey,
    kCVPixelBufferPixelFormatTypeKey, kCVPixelBufferWidthKey, kCVPixelFormatType_32BGRA,
    kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString};
use objc2_quartz_core::{CALayer, CATextLayer, kCAAlignmentLeft, kCAGravityResizeAspect};
use teslacam_mac_spike::{
    av::{self, cm, pump_until},
    fixtures::{self, CAMERA_LABELS},
    layout::{self, CAMERA_SLOTS, COMPOSITE_HEIGHT, COMPOSITE_WIDTH, Rect},
};

fn cg_rect(r: Rect) -> CGRect {
    CGRect { origin: CGPoint { x: r.x, y: r.y }, size: CGSize { width: r.width, height: r.height } }
}

fn key(k: &CFString) -> &NSString {
    unsafe { &*(k as *const CFString as *const NSString) }
}

fn load_png(path: &std::path::Path) -> CFRetained<CGImage> {
    let c = CString::new(path.to_str().unwrap()).unwrap();
    let provider = unsafe { CGDataProvider::with_filename(c.as_ptr()) }.expect("png provider");
    unsafe {
        CGImage::with_png_data_provider(Some(&provider), std::ptr::null(), true, CGColorRenderingIntent::RenderingIntentDefault)
    }
    .expect("png decode")
}

/// Static overlay (labels + vehicle) rendered once to a transparent CGImage.
fn overlay_image() -> CFRetained<CGImage> {
    let full = layout::rect(0.0, 0.0, COMPOSITE_WIDTH, COMPOSITE_HEIGHT);
    let root = CALayer::new();
    root.setFrame(cg_rect(full));
    root.setGeometryFlipped(true);
    let scrim = CGColor::new_generic_rgb(0.0, 0.0, 0.0, 0.72);
    let white = CGColor::new_generic_rgb(1.0, 1.0, 1.0, 1.0);
    let font = CFString::from_str("HiraginoSans-W5");
    for (camera, slot) in CAMERA_SLOTS {
        let label = CAMERA_LABELS[camera];
        let width = (label.chars().count() as f64 * 13.0).ceil() + 16.0;
        let box_layer = CALayer::new();
        box_layer.setFrame(cg_rect(layout::rect(slot.x + 8.0, slot.y + 8.0, width, 24.0)));
        box_layer.setBackgroundColor(Some(&scrim));
        let text = CATextLayer::new();
        text.setFrame(cg_rect(layout::rect(8.0, 4.0, width - 16.0, 18.0)));
        let s = NSString::from_str(label);
        unsafe {
            text.setString(Some(s.as_ref() as &AnyObject));
            text.setFont(Some(font.as_ref()));
        }
        text.setFontSize(13.0);
        text.setForegroundColor(Some(&white));
        text.setAlignmentMode(unsafe { kCAAlignmentLeft });
        box_layer.addSublayer(&text);
        root.addSublayer(&box_layer);
    }
    let vehicle = load_png(&fixtures::vehicle_png());
    let (w, h) = (CGImage::width(Some(&vehicle)) as f64, CGImage::height(Some(&vehicle)) as f64);
    let vehicle_layer = CALayer::new();
    vehicle_layer.setFrame(cg_rect(layout::contain_rect(w, h, layout::VEHICLE)));
    unsafe { vehicle_layer.setContents(Some(vehicle.as_ref() as &AnyObject)) };
    vehicle_layer.setContentsGravity(unsafe { kCAGravityResizeAspect });
    root.addSublayer(&vehicle_layer);

    let space = CGColorSpace::new_device_rgb().unwrap();
    // kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little
    let ctx = unsafe {
        CGBitmapContextCreate(std::ptr::null_mut(), 1280, 720, 8, 1280 * 4, Some(&space), 2 | (2 << 12))
    }
    .expect("bitmap context");
    root.renderInContext(&ctx);
    CGBitmapContextCreateImage(Some(&ctx)).expect("overlay image")
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("out path"));
    let bitrate: i64 = args.next().map(|b| b.parse().unwrap()).unwrap_or(8_000_000);
    let _ = std::fs::remove_file(&out);

    let composition = unsafe { AVMutableComposition::composition() };
    let media_type = unsafe { AVMediaTypeVideo }.unwrap();
    let mut tracks = Vec::new();
    for camera in 0..6 {
        let track = unsafe { composition.addMutableTrackWithMediaType_preferredTrackID(media_type, 0) }.unwrap();
        for segment in 0..2 {
            let asset = av::open_asset(&fixtures::clip(segment, camera));
            let source = av::load_video_track(&asset).unwrap();
            unsafe {
                track
                    .insertTimeRange_ofTrack_atTime_error(source.timeRange(), &source, cm(segment as f64 * 60.0))
                    .unwrap();
            }
        }
        tracks.push(track);
    }
    let duration = unsafe { AVAsset::duration(&composition) };
    let vc = unsafe { AVMutableVideoComposition::videoComposition() };
    unsafe {
        vc.setRenderSize(CGSize { width: COMPOSITE_WIDTH, height: COMPOSITE_HEIGHT });
        vc.setFrameDuration(CMTime::new(1, 30));
    }
    let instruction = unsafe { AVMutableVideoCompositionInstruction::videoCompositionInstruction() };
    unsafe {
        instruction.setTimeRange(CMTimeRange::new(kCMTimeZero, duration));
        instruction.setBackgroundColor(Some(&CGColor::new_generic_rgb(0.0, 0.0, 0.0, 1.0)));
    }
    let lis: Vec<Retained<AVVideoCompositionLayerInstruction>> = CAMERA_SLOTS
        .iter()
        .map(|&(camera, slot)| {
            let li = unsafe {
                AVMutableVideoCompositionLayerInstruction::videoCompositionLayerInstructionWithAssetTrack(&tracks[camera])
            };
            let [a, b, c, d, tx, ty] = layout::contain_transform(1280.0, 960.0, slot);
            unsafe { li.setTransform_atTime(CGAffineTransform { a, b, c, d, tx, ty }, kCMTimeZero) };
            Retained::into_super(li)
        })
        .collect();
    unsafe {
        instruction.setLayerInstructions(&NSArray::from_retained_slice(&lis));
        let p: Retained<ProtocolObject<dyn AVVideoCompositionInstructionProtocol>> = ProtocolObject::from_retained(instruction);
        vc.setInstructions(&NSArray::from_retained_slice(&[p]));
    }

    // Reader: composited 420f frames.
    let reader = unsafe { AVAssetReader::assetReaderWithAsset_error(&composition) }.unwrap();
    let iosurface: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::new();
    let read_format = NSNumber::new_u32(kCVPixelFormatType_420YpCbCr8BiPlanarFullRange);
    let read_settings: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::from_slices(
        &[key(unsafe { kCVPixelBufferPixelFormatTypeKey }), key(unsafe { kCVPixelBufferIOSurfacePropertiesKey })],
        &[read_format.as_ref() as &AnyObject, iosurface.as_ref() as &AnyObject],
    );
    let track_array: Retained<NSArray<objc2_av_foundation::AVAssetTrack>> =
        NSArray::from_retained_slice(&tracks.iter().map(|t| Retained::into_super(Retained::into_super(t.clone()))).collect::<Vec<_>>());
    let output = unsafe {
        AVAssetReaderVideoCompositionOutput::assetReaderVideoCompositionOutputWithVideoTracks_videoSettings(
            &track_array,
            Some(&read_settings),
        )
    };
    unsafe {
        output.setVideoComposition(Some(&vc));
        output.setAlwaysCopiesSampleData(false);
        reader.addOutput(&output);
    }

    // Writer: H.264 with explicit bitrate.
    let writer = unsafe { AVAssetWriter::assetWriterWithURL_fileType_error(&av::file_url(&out), AVFileTypeMPEG4.unwrap()) }.unwrap();
    let bitrate_n = NSNumber::new_i64(bitrate);
    let gop = NSNumber::new_i32(30);
    let compression: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::from_slices(
        &[unsafe { AVVideoAverageBitRateKey }.unwrap(), unsafe { AVVideoMaxKeyFrameIntervalKey }.unwrap()],
        &[bitrate_n.as_ref() as &AnyObject, gop.as_ref() as &AnyObject],
    );
    let w = NSNumber::new_i32(1280);
    let h = NSNumber::new_i32(720);
    let codec: &NSString = unsafe { AVVideoCodecTypeH264 }.unwrap();
    let write_settings: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::from_slices(
        &[
            unsafe { AVVideoCodecKey }.unwrap(),
            unsafe { AVVideoWidthKey }.unwrap(),
            unsafe { AVVideoHeightKey }.unwrap(),
            unsafe { AVVideoCompressionPropertiesKey }.unwrap(),
        ],
        &[codec as &AnyObject, w.as_ref() as &AnyObject, h.as_ref() as &AnyObject, compression.as_ref() as &AnyObject],
    );
    let input = unsafe { AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings(media_type, Some(&write_settings)) };
    unsafe { input.setExpectsMediaDataInRealTime(false) };
    let bgra = NSNumber::new_u32(kCVPixelFormatType_32BGRA);
    let adaptor_attrs: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::from_slices(
        &[
            key(unsafe { kCVPixelBufferPixelFormatTypeKey }),
            key(unsafe { kCVPixelBufferWidthKey }),
            key(unsafe { kCVPixelBufferHeightKey }),
            key(unsafe { kCVPixelBufferIOSurfacePropertiesKey }),
        ],
        &[bgra.as_ref() as &AnyObject, w.as_ref() as &AnyObject, h.as_ref() as &AnyObject, iosurface.as_ref() as &AnyObject],
    );
    let adaptor = unsafe {
        AVAssetWriterInputPixelBufferAdaptor::assetWriterInputPixelBufferAdaptorWithAssetWriterInput_sourcePixelBufferAttributes(
            &input,
            Some(&adaptor_attrs),
        )
    };
    unsafe { writer.addInput(&input) };

    let overlay = unsafe { CIImage::imageWithCGImage(&overlay_image()) };
    let context = unsafe { CIContext::contextWithOptions(None) };

    let t0 = Instant::now();
    assert!(unsafe { reader.startReading() }, "startReading");
    assert!(unsafe { writer.startWriting() }, "startWriting");
    unsafe { writer.startSessionAtSourceTime(kCMTimeZero) };
    let pool: Retained<CVPixelBufferPool> = unsafe { adaptor.pixelBufferPool() }.expect("pool");
    let mut frames = 0u64;
    while let Some(sample) = unsafe { output.copyNextSampleBuffer() } {
        let pts = unsafe { sample.presentation_time_stamp() };
        let source: CFRetained<CVPixelBuffer> = unsafe { sample.image_buffer() }.expect("image buffer");
        let base = unsafe { CIImage::imageWithCVPixelBuffer(&source) };
        let composed = unsafe { overlay.imageByCompositingOverImage(&base) };
        let mut dst: *mut CVPixelBuffer = std::ptr::null_mut();
        unsafe { CVPixelBufferPool::create_pixel_buffer(None, &pool, NonNull::from(&mut dst)) };
        let dst = unsafe { CFRetained::from_raw(NonNull::new(dst).expect("pool buffer")) };
        unsafe { context.render_toCVPixelBuffer(&composed, &dst) };
        while !unsafe { input.isReadyForMoreMediaData() } {
            std::thread::sleep(Duration::from_micros(500));
        }
        assert!(unsafe { adaptor.appendPixelBuffer_withPresentationTime(&dst, pts) }, "append");
        frames += 1;
    }
    unsafe { input.markAsFinished() };
    let done = Arc::new(AtomicBool::new(false));
    let flag = done.clone();
    let handler = RcBlock::new(move || flag.store(true, Ordering::SeqCst));
    unsafe { writer.finishWritingWithCompletionHandler(&handler) };
    pump_until(Duration::from_secs(60), || done.load(Ordering::SeqCst));
    let elapsed = t0.elapsed().as_secs_f64();
    let media = av::secs(duration);
    println!(
        "EXPORT_RW frames={frames} wall_s={elapsed:.2} media_s={media:.2} x_realtime={:.2} writer_status={:?} reader_status={:?} error={:?} bitrate={bitrate}",
        media / elapsed,
        unsafe { writer.status() },
        unsafe { reader.status() },
        unsafe { writer.error() }.map(|e| e.localizedDescription().to_string()),
    );
}
