//! Q6: composite export with AVMutableComposition (6 tracks) +
//! AVMutableVideoComposition (contain-fit layer transforms) +
//! AVVideoCompositionCoreAnimationTool (labels + vehicle image) ->
//! AVAssetExportSession (H.264 MP4).
//!
//! Usage: export_probe <out.mp4> [ca|plain] [preset: 1280x720|highest]
use std::{
    ffi::CString,
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

use block2::RcBlock;
use objc2::{AnyThread, MainThreadMarker, rc::Retained, runtime::{AnyObject, ProtocolObject}};
use objc2_av_foundation::{
    AVAsset, AVAssetExportPreset1280x720, AVAssetExportPresetHighestQuality, AVAssetExportSession,
    AVFileTypeMPEG4, AVMediaTypeVideo, AVMutableComposition, AVMutableVideoComposition,
    AVMutableVideoCompositionInstruction, AVMutableVideoCompositionLayerInstruction,
    AVVideoCompositionCoreAnimationTool, AVVideoCompositionInstructionProtocol,
    AVVideoCompositionLayerInstruction,
};
use objc2_core_foundation::{CFString, CGAffineTransform, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGColor, CGColorRenderingIntent, CGDataProvider, CGImage};
use objc2_core_media::{CMTime, CMTimeRange, kCMTimeZero};
use objc2_foundation::{NSArray, NSString};
use objc2_quartz_core::{CALayer, CATextLayer, kCAAlignmentLeft, kCAGravityResizeAspect};
use teslacam_mac_spike::{
    av::{self, cm, pump_until},
    fixtures::{self, CAMERA_LABELS},
    layout::{self, CAMERA_SLOTS, COMPOSITE_HEIGHT, COMPOSITE_WIDTH, Rect},
};

fn cg_rect(r: Rect) -> CGRect {
    CGRect { origin: CGPoint { x: r.x, y: r.y }, size: CGSize { width: r.width, height: r.height } }
}

fn load_png(path: &std::path::Path) -> objc2_core_foundation::CFRetained<CGImage> {
    let c = CString::new(path.to_str().unwrap()).unwrap();
    let provider = unsafe { CGDataProvider::with_filename(c.as_ptr()) }.expect("png provider");
    unsafe {
        CGImage::with_png_data_provider(Some(&provider), std::ptr::null(), true, CGColorRenderingIntent::RenderingIntentDefault)
    }
    .expect("png decode")
}

fn overlay_layers() -> (Retained<CALayer>, Retained<CALayer>) {
    let full = layout::rect(0.0, 0.0, COMPOSITE_WIDTH, COMPOSITE_HEIGHT);
    let parent = CALayer::new();
    parent.setFrame(cg_rect(full));
    // Use top-left origin like the web canvas.
    parent.setGeometryFlipped(true);
    let video = CALayer::new();
    video.setFrame(cg_rect(full));
    parent.addSublayer(&video);

    let scrim = CGColor::new_generic_rgb(0.0, 0.0, 0.0, 0.72);
    let white = CGColor::new_generic_rgb(1.0, 1.0, 1.0, 1.0);
    let font = CFString::from_str("HiraginoSans-W5");
    for (camera, slot) in CAMERA_SLOTS {
        let label = CAMERA_LABELS[camera];
        // Web tokens: inset 8, padding 8, height 24, 13px font, baseline 17.
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
        text.setContentsScale(1.0);
        box_layer.addSublayer(&text);
        parent.addSublayer(&box_layer);
        if std::env::var("SPIKE_CA_DISPLAY").is_ok_and(|v| v == "1") {
            // Render the text backing store now; the export renders the layer
            // tree off the main thread and never runs a display pass itself.
            text.display();
        }
    }
    let vehicle = load_png(&fixtures::vehicle_png());
    let (w, h) = (CGImage::width(Some(&vehicle)) as f64, CGImage::height(Some(&vehicle)) as f64);
    let fitted = layout::contain_rect(w, h, layout::VEHICLE);
    let vehicle_layer = CALayer::new();
    vehicle_layer.setFrame(cg_rect(fitted));
    unsafe { vehicle_layer.setContents(Some(vehicle.as_ref() as &AnyObject)) };
    vehicle_layer.setContentsGravity(unsafe { kCAGravityResizeAspect });
    parent.addSublayer(&vehicle_layer);
    if std::env::var("SPIKE_CA_DISPLAY").is_ok_and(|v| v == "1") {
        objc2_quartz_core::CATransaction::flush();
    }
    (video, parent)
}

fn main() {
    let _mtm = MainThreadMarker::new().unwrap();
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("out path"));
    let mode = args.next().unwrap_or_else(|| "ca".into());
    let preset = args.next().unwrap_or_else(|| "1280x720".into());
    let _ = std::fs::remove_file(&out);

    let t_build = Instant::now();
    let composition = unsafe { AVMutableComposition::composition() };
    let media_type = unsafe { AVMediaTypeVideo }.unwrap();
    let mut tracks = Vec::new();
    for camera in 0..6 {
        let track = unsafe { composition.addMutableTrackWithMediaType_preferredTrackID(media_type, 0) }
            .expect("track");
        for segment in 0..2 {
            let asset = av::open_asset(&fixtures::clip(segment, camera));
            let source = av::load_video_track(&asset).expect("source track");
            let range = unsafe { source.timeRange() };
            unsafe {
                track
                    .insertTimeRange_ofTrack_atTime_error(range, &source, cm(segment as f64 * 60.0))
                    .expect("insert");
            }
        }
        tracks.push(track);
    }
    let duration = unsafe { AVAsset::duration(&composition) };

    let video_composition = unsafe { AVMutableVideoComposition::videoComposition() };
    unsafe {
        video_composition.setRenderSize(CGSize { width: COMPOSITE_WIDTH, height: COMPOSITE_HEIGHT });
        video_composition.setFrameDuration(CMTime::new(1, 30));
    }
    let instruction = unsafe { AVMutableVideoCompositionInstruction::videoCompositionInstruction() };
    unsafe {
        instruction.setTimeRange(CMTimeRange::new(kCMTimeZero, duration));
        instruction.setBackgroundColor(Some(&CGColor::new_generic_rgb(0.0, 0.0, 0.0, 1.0)));
    }
    let layer_instructions: Vec<Retained<AVVideoCompositionLayerInstruction>> = CAMERA_SLOTS
        .iter()
        .map(|&(camera, slot)| {
            let li = unsafe {
                AVMutableVideoCompositionLayerInstruction::videoCompositionLayerInstructionWithAssetTrack(
                    &tracks[camera],
                )
            };
            let [a, b, c, d, tx, ty] = layout::contain_transform(1280.0, 960.0, slot);
            unsafe { li.setTransform_atTime(CGAffineTransform { a, b, c, d, tx, ty }, kCMTimeZero) };
            Retained::into_super(li)
        })
        .collect();
    unsafe {
        instruction.setLayerInstructions(&NSArray::from_retained_slice(&layer_instructions));
        let instruction: Retained<ProtocolObject<dyn AVVideoCompositionInstructionProtocol>> =
            ProtocolObject::from_retained(instruction);
        video_composition.setInstructions(&NSArray::from_retained_slice(&[instruction]));
    }
    if mode == "ca" {
        let (video, parent) = overlay_layers();
        let tool = unsafe {
            AVVideoCompositionCoreAnimationTool::videoCompositionCoreAnimationToolWithPostProcessingAsVideoLayer_inLayer(
                &video, &parent,
            )
        };
        unsafe { video_composition.setAnimationTool(Some(&tool)) };
    }
    let build_ms = t_build.elapsed().as_secs_f64() * 1000.0;

    let preset_name = match preset.as_str() {
        "highest" => unsafe { AVAssetExportPresetHighestQuality },
        _ => unsafe { AVAssetExportPreset1280x720 },
    };
    let session = unsafe {
        AVAssetExportSession::initWithAsset_presetName(
            AVAssetExportSession::alloc(),
            &composition,
            preset_name,
        )
    }
    .expect("export session");
    unsafe {
        session.setOutputURL(Some(&av::file_url(&out)));
        session.setOutputFileType(AVFileTypeMPEG4);
        session.setVideoComposition(Some(&video_composition));
    }
    let done = Arc::new(AtomicBool::new(false));
    let flag = done.clone();
    let handler = RcBlock::new(move || flag.store(true, Ordering::SeqCst));
    let t0 = Instant::now();
    unsafe { session.exportAsynchronouslyWithCompletionHandler(&handler) };
    let mut last_report = Instant::now();
    pump_until(Duration::from_secs(900), || {
        if last_report.elapsed() > Duration::from_secs(5) {
            last_report = Instant::now();
            eprintln!("progress {:.1}%", unsafe { session.progress() } * 100.0);
        }
        done.load(Ordering::SeqCst)
    });
    let elapsed = t0.elapsed().as_secs_f64();
    let status = unsafe { session.status() };
    let error = unsafe { session.error() }.map(|e| e.localizedDescription().to_string());
    let media = av::secs(duration);
    println!(
        "EXPORT mode={mode} preset={preset} build_ms={build_ms:.1} wall_s={elapsed:.2} media_s={media:.2} x_realtime={:.2} status={:?} error={error:?} out={}",
        media / elapsed,
        status,
        out.display()
    );
}
