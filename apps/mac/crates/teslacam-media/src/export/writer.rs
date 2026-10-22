//! [`ExportBackend::ReaderWriter`](super::ExportBackend::ReaderWriter): the default pipeline.
//!
//! - Every camera with video gets a reader thread: an `AVAssetReader` over the camera's composition
//!   (its clips on the event axis, as for playback) decodes in hardware and hands `420v` frames to
//!   the encoder through a small bounded channel. Decoding six cameras is the bottleneck, so the six
//!   decoders run in parallel.
//! - The encoder (the export's worker thread) walks the output frames: a [`FrameCursor`] per camera
//!   picks the frame due at `n / fps`, Core Image draws the floor, the cameras contain-fitted into
//!   their slots, the missing tiles and the overlay into a buffer from the writer's pool on the GPU,
//!   and `AVAssetWriter` encodes it with the hardware H.264 encoder.
//!
//! Nothing here runs on the main thread: [`Pending::start`] only tells the worker to begin, and the
//! job then reads an atomic frame counter and a channel.

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    ptr::NonNull,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use block2::RcBlock;
use objc2::{
    AnyThread, Message,
    rc::{Retained, autoreleasepool},
    runtime::AnyObject,
};
use objc2_av_foundation::{
    AVAssetReader, AVAssetReaderStatus, AVAssetReaderTrackOutput, AVAssetTrack, AVAssetWriter,
    AVAssetWriterInput, AVAssetWriterInputPixelBufferAdaptor, AVAssetWriterStatus, AVFileTypeMPEG4,
    AVMediaTypeVideo, AVMutableComposition, AVVideoAverageBitRateKey, AVVideoCodecKey,
    AVVideoCodecTypeH264, AVVideoColorPrimaries_ITU_R_709_2, AVVideoColorPrimariesKey,
    AVVideoColorPropertiesKey, AVVideoCompressionPropertiesKey, AVVideoExpectedSourceFrameRateKey,
    AVVideoHeightKey, AVVideoMaxKeyFrameIntervalKey, AVVideoProfileLevelH264MainAutoLevel,
    AVVideoProfileLevelKey, AVVideoTransferFunction_ITU_R_709_2, AVVideoTransferFunctionKey,
    AVVideoWidthKey, AVVideoYCbCrMatrix_ITU_R_709_2, AVVideoYCbCrMatrixKey,
};
use objc2_core_foundation::{CFRetained, CGAffineTransform, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGColorSpace, kCGColorSpaceSRGB};
use objc2_core_image::{
    CIColor, CIContext, CIImage, CIRenderDestination, CIRenderTask, kCIContextCacheIntermediates,
    kCIContextWorkingColorSpace,
};
use objc2_core_media::{CMSampleBuffer, CMTime};
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferPool, kCVPixelBufferHeightKey,
    kCVPixelBufferIOSurfacePropertiesKey, kCVPixelBufferMetalCompatibilityKey,
    kCVPixelBufferPixelFormatTypeKey, kCVPixelBufferWidthKey, kCVPixelFormatType_32BGRA,
    kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange, kCVReturnSuccess,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString};

use super::{
    Encoding, ExportError, Prepared, failure,
    frames::{FrameCursor, clip_at},
    media_failure,
};
use crate::{
    av::{self, SendBox},
    input::{CameraId, CameraMap},
    layout::{Affine, ExportLayout, Rect, bottom_left_transform, fit_transform},
};

/// Average H.264 bit rate, what `AVAssetExportPreset1280x720` wrote for the fixture event (v0.1.0),
/// inside the 4–8 Mbit/s the spec recommends (§3.5).
const BIT_RATE: i64 = 7_300_000;
/// Seconds between key frames (spec §3.5, as on the web).
const KEY_FRAME_INTERVAL: u32 = 2;
/// Decoded frames a camera's reader may run ahead of the encoder.
const READ_AHEAD: usize = 4;
/// Frames the GPU may render ahead of the encoder. The GPU is shared with every other app; waiting
/// for each frame before composing the next one left it idle between frames, and on a busy machine
/// compositing then took 12.5 s instead of overlapping with decoding.
const RENDER_AHEAD: usize = 3;

/// Shared between the worker thread and the main thread.
struct Shared {
    /// Frames appended to the writer so far.
    frames: AtomicU64,
    cancel: Arc<AtomicBool>,
}

/// The main thread's side of a prepared export: [`Pending::start`] lets the worker write the file.
pub(super) struct Pending {
    go: mpsc::Sender<()>,
    shared: Arc<Shared>,
    frame_count: u64,
    done: mpsc::Receiver<Result<(), ExportError>>,
}

/// Where a camera's frames are drawn, in Core Image coordinates (origin at the bottom-left).
struct Placement {
    /// Event start of each clip, with the transform that contain-fits that clip into the slot.
    starts: Vec<f64>,
    transforms: Vec<Affine>,
    slot: CGRect,
}

/// A camera with video: what its reader reads and where its frames go.
struct CameraSource {
    composition: SendBox<Retained<AVMutableComposition>>,
    placement: Placement,
}

/// The worker thread's side.
pub(super) struct Encoder {
    prepared: Prepared,
    cameras: CameraMap<Option<CameraSource>>,
    layout: ExportLayout,
    output: PathBuf,
    go: mpsc::Receiver<()>,
    shared: Arc<Shared>,
    done: mpsc::Sender<Result<(), ExportError>>,
}

/// Worker thread: everything the encoder needs before it may touch `output` (a failure here keeps an
/// existing file).
pub(super) fn compose(
    prepared: Prepared,
    layout: ExportLayout,
    output: PathBuf,
    cancel: Arc<AtomicBool>,
) -> Result<(Pending, Encoder), ExportError> {
    let mut cameras = CameraMap::from_fn(|_| None);
    for camera in CameraId::ALL {
        let camera_timeline = &prepared.timeline.cameras[camera];
        if !camera_timeline.has_video() {
            continue;
        }
        let sources = prepared.sources(camera);
        let refs: Vec<&AVAssetTrack> = sources.iter().map(|(_, track)| &**track).collect();
        let composition =
            av::camera_composition(camera_timeline, &refs, prepared.timeline.duration)
                .map_err(media_failure)?;
        let slot = layout.slots[camera];
        let (starts, transforms) = sources
            .iter()
            .map(|(start, track)| {
                let (natural, preferred) =
                    unsafe { (track.naturalSize(), track.preferredTransform()) };
                let fit = fit_transform(
                    (natural.width, natural.height),
                    av::affine_parts(preferred),
                    slot,
                );
                (
                    *start,
                    bottom_left_transform(fit, natural.height, layout.height),
                )
            })
            .unzip();
        cameras[camera] = Some(CameraSource {
            composition: SendBox(composition),
            placement: Placement {
                starts,
                transforms,
                slot: cg_rect(slot.bottom_left(layout.height)),
            },
        });
    }
    let (go_tx, go_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let shared = Arc::new(Shared {
        frames: AtomicU64::new(0),
        cancel,
    });
    let pending = Pending {
        go: go_tx,
        shared: shared.clone(),
        frame_count: prepared.plan.frame_count,
        done: done_rx,
    };
    let encoder = Encoder {
        prepared,
        cameras,
        layout,
        output,
        go: go_rx,
        shared,
        done: done_tx,
    };
    Ok((pending, encoder))
}

impl Pending {
    /// Main thread: lets the worker replace the output file and encode.
    pub fn start(self) -> Box<dyn Encoding> {
        let _ = self.go.send(());
        Box::new(Running {
            shared: self.shared,
            frame_count: self.frame_count,
            done: self.done,
        })
    }
}

impl Encoder {
    /// Worker thread: waits for [`Pending::start`] (and returns at once when the job ended instead),
    /// then encodes and reports the outcome. Partial output is removed before a failure is reported.
    pub fn run(self) {
        let Encoder {
            prepared,
            cameras,
            layout,
            output,
            go,
            shared,
            done,
        } = self;
        if go.recv().is_err() {
            return;
        }
        let outcome = encode(&prepared, cameras, &layout, &output, &shared);
        if outcome.is_err() {
            let _ = std::fs::remove_file(&output);
        }
        let _ = done.send(outcome);
    }
}

fn encode(
    prepared: &Prepared,
    mut cameras: CameraMap<Option<CameraSource>>,
    layout: &ExportLayout,
    output: &Path,
    shared: &Shared,
) -> Result<(), ExportError> {
    let plan = &prepared.plan;
    // The save panel confirmed replacing the file, and AVAssetWriter does not overwrite.
    let _ = std::fs::remove_file(output);
    let writer = Writer::start(output, layout)?;
    let compositor = Compositor::new(prepared, layout)?;
    let mut streams: CameraMap<Option<Stream>> = CameraMap::from_fn(|_| None);
    let mut placements: CameraMap<Option<Placement>> = CameraMap::from_fn(|_| None);
    for camera in CameraId::ALL {
        if let Some(source) = cameras[camera].take() {
            streams[camera] = Some(Stream::spawn(source.composition, shared.cancel.clone()));
            placements[camera] = Some(source.placement);
        }
    }
    let fps = f64::from(layout.fps);
    let mut rendering: VecDeque<Rendering> = VecDeque::with_capacity(RENDER_AHEAD + 1);
    let deliver = |rendering: Rendering| -> Result<(), ExportError> {
        unsafe { rendering.task.waitUntilCompletedAndReturnError() }
            .map_err(|error| media_failure(av::media_error(&error)))?;
        writer.append(&rendering.buffer, rendering.index, &shared.cancel)?;
        shared.frames.store(rendering.index + 1, Ordering::SeqCst);
        Ok(())
    };
    for index in 0..plan.frame_count {
        if shared.cancel.load(Ordering::SeqCst) {
            return Err(ExportError::Aborted);
        }
        let t = index as f64 / fps;
        autoreleasepool(|_| {
            let mut image = compositor.background.clone();
            for camera in CameraId::ALL {
                if let (Some(stream), Some(placement)) = (&mut streams[camera], &placements[camera])
                    && let Some(frame) = stream.frame_at(t)?
                {
                    image = compositor.video(&image, frame, placement);
                }
                if plan.cameras[camera].missing.visible_at(t) {
                    image = compositor.missing(&image, camera);
                }
            }
            let buffer = writer.buffer()?;
            let task = compositor.render(&image, &buffer)?;
            rendering.push_back(Rendering {
                index,
                buffer,
                task,
            });
            match rendering.len() > RENDER_AHEAD {
                true => deliver(rendering.pop_front().expect("more than RENDER_AHEAD")),
                false => Ok(()),
            }
        })?;
    }
    while let Some(next) = rendering.pop_front() {
        deliver(next)?;
    }
    writer.finish(plan.frame_count)
}

/// An output frame the GPU is drawing.
struct Rendering {
    index: u64,
    buffer: CFRetained<CVPixelBuffer>,
    task: Retained<CIRenderTask>,
}

/// One decoded frame and its presentation time on the event axis.
struct Frame {
    time: f64,
    buffer: CFRetained<CVPixelBuffer>,
}

// SAFETY: CVPixelBuffers are reference counted and safe to use from any thread; the reader thread
// hands each frame over and never touches it again.
unsafe impl Send for Frame {}

type Decoded = Result<Frame, ExportError>;

/// A camera's reader thread and the frames it decoded.
struct Stream {
    frames: Option<mpsc::Receiver<Decoded>>,
    thread: Option<JoinHandle<()>>,
    cursor: FrameCursor<Frame>,
}

impl Stream {
    fn spawn(
        composition: SendBox<Retained<AVMutableComposition>>,
        cancel: Arc<AtomicBool>,
    ) -> Stream {
        let (tx, rx) = mpsc::sync_channel(READ_AHEAD);
        let thread = thread::spawn(move || {
            av::run_as_user_initiated();
            let composition = composition.into_inner();
            read(&composition, &tx, &cancel);
        });
        Stream {
            frames: Some(rx),
            thread: Some(thread),
            cursor: FrameCursor::new(),
        }
    }

    /// The frame to show at output time `t`, waiting for the decoder when needed.
    fn frame_at(&mut self, t: f64) -> Result<Option<&Frame>, ExportError> {
        let Stream { frames, cursor, .. } = self;
        let frames = frames.as_ref().expect("open until dropped");
        cursor.advance(t, || match frames.recv() {
            Ok(Ok(frame)) => Ok(Some((frame.time, frame))),
            Ok(Err(error)) => Err(error),
            // The reader finished (or stopped because the export is cancelled).
            Err(mpsc::RecvError) => Ok(None),
        })
    }
}

impl Drop for Stream {
    fn drop(&mut self) {
        // Hanging up stops the reader at its next frame.
        self.frames = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Reader thread: decodes the camera's composition until it ends, fails, the export is cancelled or
/// the encoder hangs up.
fn read(composition: &AVMutableComposition, tx: &mpsc::SyncSender<Decoded>, cancel: &AtomicBool) {
    let (reader, output) = match autoreleasepool(|_| open_reader(composition)) {
        Ok(opened) => opened,
        Err(error) => {
            let _ = tx.send(Err(error));
            return;
        }
    };
    loop {
        if cancel.load(Ordering::SeqCst) {
            unsafe { reader.cancelReading() };
            return;
        }
        let next = autoreleasepool(|_| unsafe { output.copyNextSampleBuffer() }.map(|s| frame(&s)));
        let sent = match next {
            Some(Some(frame)) => tx.send(Ok(frame)),
            // A sample without an image carries nothing to show.
            Some(None) => continue,
            None => {
                if unsafe { reader.status() } == AVAssetReaderStatus::Failed {
                    let error = unsafe { reader.error() }.map(|e| av::media_error(&e));
                    let _ = tx.send(Err(ExportError::RecordingFailed(error)));
                }
                return;
            }
        };
        if sent.is_err() {
            unsafe { reader.cancelReading() };
            return;
        }
    }
}

fn open_reader(
    composition: &AVMutableComposition,
) -> Result<(Retained<AVAssetReader>, Retained<AVAssetReaderTrackOutput>), ExportError> {
    let reader = unsafe { AVAssetReader::assetReaderWithAsset_error(composition) }
        .map_err(|error| media_failure(av::media_error(&error)))?;
    let track = unsafe { composition.tracks() }
        .firstObject()
        .ok_or_else(|| failure("a camera composition has no track"))?;
    // The decoder's own format: no conversion before Core Image reads it on the GPU.
    let format = NSNumber::new_u32(kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange);
    let iosurface: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::new();
    let yes = NSNumber::new_bool(true);
    let settings = dictionary(&[
        (
            av::ns_key(unsafe { kCVPixelBufferPixelFormatTypeKey }),
            object(&*format),
        ),
        (
            av::ns_key(unsafe { kCVPixelBufferIOSurfacePropertiesKey }),
            object(&*iosurface),
        ),
        (
            av::ns_key(unsafe { kCVPixelBufferMetalCompatibilityKey }),
            object(&*yes),
        ),
    ]);
    let output = unsafe {
        AVAssetReaderTrackOutput::assetReaderTrackOutputWithTrack_outputSettings(
            &track,
            Some(&settings),
        )
    };
    unsafe {
        output.setAlwaysCopiesSampleData(false);
        if !reader.canAddOutput(&output) {
            return Err(failure("could not read a camera composition"));
        }
        reader.addOutput(&output);
        if !reader.startReading() {
            let error = reader.error().map(|e| av::media_error(&e));
            return Err(ExportError::RecordingFailed(error));
        }
    }
    Ok((reader, output))
}

fn frame(sample: &CMSampleBuffer) -> Option<Frame> {
    let time = av::secs(unsafe { sample.presentation_time_stamp() });
    let buffer = unsafe { sample.image_buffer() }?;
    Some(Frame { time, buffer })
}

/// Draws output frames with Core Image on the GPU.
struct Compositor {
    context: Retained<CIContext>,
    /// `CIColorClamp`, whose defaults clip every component to 0…1.
    clamp: Retained<NSString>,
    space: CFRetained<CGColorSpace>,
    /// The floor colour over the whole frame.
    background: Retained<CIImage>,
    overlay: Retained<CIImage>,
    /// Each camera's missing tile, placed over its window.
    missing: CameraMap<Option<Retained<CIImage>>>,
}

impl Compositor {
    fn new(prepared: &Prepared, layout: &ExportLayout) -> Result<Compositor, ExportError> {
        let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }))
            .ok_or_else(|| failure("no sRGB colour space"))?;
        let frame = cg_rect(Rect::new(0.0, 0.0, layout.width, layout.height));
        let floor = unsafe {
            CIColor::colorWithRed_green_blue_alpha_colorSpace(
                layout.floor.r,
                layout.floor.g,
                layout.floor.b,
                1.0,
                &space,
            )
        }
        .ok_or_else(|| failure("could not make the floor colour"))?;
        let background = unsafe { CIImage::imageWithColor(&floor).imageByCroppingToRect(frame) };
        let overlay = unsafe { CIImage::imageWithCGImage(&prepared.overlay.image) };
        let missing = prepared.missing.map(|camera, tile| {
            let tile = tile.as_ref()?;
            let hole = prepared.plan.cameras[camera]
                .hole
                .bottom_left(layout.height);
            let image = unsafe { CIImage::imageWithCGImage(&tile.image) };
            let (width, height) = (tile.bitmap.width as f64, tile.bitmap.height as f64);
            let fit = [
                hole.width / width,
                0.0,
                0.0,
                hole.height / height,
                hole.x,
                hole.y,
            ];
            Some(unsafe { image.imageByApplyingTransform(cg_affine(fit)) })
        });
        // Blend in sRGB like the browser canvas and Core Animation (the label scrim and the anti-aliased
        // hole edges look the same as on the web), and do not keep per-frame intermediates.
        let no = NSNumber::new_bool(false);
        let options = dictionary(&[
            (unsafe { kCIContextCacheIntermediates }, object(&*no)),
            (unsafe { kCIContextWorkingColorSpace }, object(&*space)),
        ]);
        let context = unsafe { CIContext::contextWithOptions(Some(&options)) };
        Ok(Compositor {
            context,
            clamp: NSString::from_str("CIColorClamp"),
            space,
            background,
            overlay,
            missing,
        })
    }

    /// `under` with a camera frame contain-fitted into its slot on top.
    fn video(&self, under: &CIImage, frame: &Frame, placement: &Placement) -> Retained<CIImage> {
        let transform = placement.transforms[clip_at(&placement.starts, frame.time)];
        unsafe {
            CIImage::imageWithCVPixelBuffer(&frame.buffer)
                // Saturated Y'CbCr colours convert to RGB beyond 0…1 in Core Image's extended
                // working range. Shown directly they clip, but under the label scrim the excess would
                // show through (a brighter pill than on the web), so clip them first.
                .imageByApplyingFilter(&self.clamp)
                .imageByApplyingTransform(cg_affine(transform))
                .imageByCroppingToRect(placement.slot)
                .imageByCompositingOverImage(under)
        }
    }

    /// `under` with `camera`'s missing tile on top.
    fn missing(&self, under: &CIImage, camera: CameraId) -> Retained<CIImage> {
        match &self.missing[camera] {
            Some(tile) => unsafe { tile.imageByCompositingOverImage(under) },
            None => under.retain(),
        }
    }

    /// Starts drawing `image` with the overlay on top into `buffer` on the GPU.
    fn render(
        &self,
        image: &CIImage,
        buffer: &CVPixelBuffer,
    ) -> Result<Retained<CIRenderTask>, ExportError> {
        unsafe {
            let image = self.overlay.imageByCompositingOverImage(image);
            let destination =
                CIRenderDestination::initWithPixelBuffer(CIRenderDestination::alloc(), buffer);
            destination.setColorSpace(Some(&self.space));
            self.context
                .startTaskToRender_toDestination_error(&image, &destination)
                .map_err(|error| media_failure(av::media_error(&error)))
        }
    }
}

/// `AVAssetWriter` with one H.264 input fed from a pool of BGRA buffers.
struct Writer {
    writer: Retained<AVAssetWriter>,
    input: Retained<AVAssetWriterInput>,
    adaptor: Retained<AVAssetWriterInputPixelBufferAdaptor>,
    pool: Retained<CVPixelBufferPool>,
    fps: i32,
}

impl Writer {
    fn start(output: &Path, layout: &ExportLayout) -> Result<Writer, ExportError> {
        let file_type = unsafe { AVFileTypeMPEG4 }.expect("AVFileTypeMPEG4 is always defined");
        let writer = unsafe {
            AVAssetWriter::assetWriterWithURL_fileType_error(&av::file_url(output), file_type)
        }
        .map_err(|error| media_failure(av::media_error(&error)))?;
        let media_type = unsafe { AVMediaTypeVideo }.expect("AVMediaTypeVideo is always defined");
        let input = unsafe {
            AVAssetWriterInput::assetWriterInputWithMediaType_outputSettings(
                media_type,
                Some(&video_settings(layout)),
            )
        };
        let adaptor = unsafe {
            AVAssetWriterInputPixelBufferAdaptor::assetWriterInputPixelBufferAdaptorWithAssetWriterInput_sourcePixelBufferAttributes(
                &input,
                Some(&buffer_attributes(layout)),
            )
        };
        let fps = layout.fps as i32;
        unsafe {
            input.setExpectsMediaDataInRealTime(false);
            // The time scale AVAssetExportSession wrote.
            input.setMediaTimeScale(600);
            if !writer.canAddInput(&input) {
                return Err(failure("could not add the video input"));
            }
            writer.addInput(&input);
            // Like AVAssetExportSession: the index (moov) first, so players can start at once.
            writer.setShouldOptimizeForNetworkUse(true);
            if !writer.startWriting() {
                return Err(writer_error(&writer));
            }
            writer.startSessionAtSourceTime(CMTime::new(0, fps));
        }
        let Some(pool) = (unsafe { adaptor.pixelBufferPool() }) else {
            unsafe { writer.cancelWriting() };
            return Err(failure("the writer has no pixel buffer pool"));
        };
        Ok(Writer {
            writer,
            input,
            adaptor,
            pool,
            fps,
        })
    }

    fn buffer(&self) -> Result<CFRetained<CVPixelBuffer>, ExportError> {
        let mut buffer: *mut CVPixelBuffer = std::ptr::null_mut();
        let status = unsafe {
            CVPixelBufferPool::create_pixel_buffer(None, &self.pool, NonNull::from(&mut buffer))
        };
        match NonNull::new(buffer) {
            // SAFETY: the pool returned a +1 buffer.
            Some(buffer) if status == kCVReturnSuccess => {
                Ok(unsafe { CFRetained::from_raw(buffer) })
            }
            _ => Err(failure("could not get a pixel buffer")),
        }
    }

    /// Appends `buffer` as output frame `index`, waiting while the encoder is busy.
    fn append(
        &self,
        buffer: &CVPixelBuffer,
        index: u64,
        cancel: &AtomicBool,
    ) -> Result<(), ExportError> {
        while !unsafe { self.input.isReadyForMoreMediaData() } {
            if cancel.load(Ordering::SeqCst) {
                return Err(ExportError::Aborted);
            }
            if unsafe { self.writer.status() } != AVAssetWriterStatus::Writing {
                return Err(self.error());
            }
            thread::sleep(Duration::from_micros(500));
        }
        let time = unsafe { CMTime::new(index as i64, self.fps) };
        if unsafe {
            self.adaptor
                .appendPixelBuffer_withPresentationTime(buffer, time)
        } {
            Ok(())
        } else {
            Err(self.error())
        }
    }

    /// Ends the video after `frame_count` frames and waits until the file is complete.
    fn finish(self, frame_count: u64) -> Result<(), ExportError> {
        unsafe {
            self.input.markAsFinished();
            self.writer
                .endSessionAtSourceTime(CMTime::new(frame_count as i64, self.fps));
        }
        let (tx, rx) = mpsc::channel();
        let handler = RcBlock::new(move || {
            let _ = tx.send(());
        });
        unsafe { self.writer.finishWritingWithCompletionHandler(&handler) };
        let _ = rx.recv();
        if unsafe { self.writer.status() } == AVAssetWriterStatus::Completed {
            Ok(())
        } else {
            Err(self.error())
        }
    }

    fn error(&self) -> ExportError {
        writer_error(&self.writer)
    }
}

fn writer_error(writer: &AVAssetWriter) -> ExportError {
    let error = unsafe { writer.error() }.map(|e| av::media_error(&e));
    ExportError::RecordingFailed(error.or_else(|| Some(av::local_error("the writer stopped"))))
}

impl Drop for Writer {
    fn drop(&mut self) {
        if unsafe { self.writer.status() } == AVAssetWriterStatus::Writing {
            unsafe { self.writer.cancelWriting() };
        }
    }
}

/// H.264 Main with BT.709 tags like the v0.1.0 export, a key frame every 2 s (spec §3.5).
fn video_settings(layout: &ExportLayout) -> Retained<NSDictionary<NSString, AnyObject>> {
    let fps = NSNumber::new_u32(layout.fps);
    let bit_rate = NSNumber::new_i64(BIT_RATE);
    let key_frames = NSNumber::new_u32(KEY_FRAME_INTERVAL * layout.fps);
    let compression = dictionary(&[
        (
            av_key(unsafe { AVVideoAverageBitRateKey }),
            object(&*bit_rate),
        ),
        (
            av_key(unsafe { AVVideoMaxKeyFrameIntervalKey }),
            object(&*key_frames),
        ),
        (
            av_key(unsafe { AVVideoExpectedSourceFrameRateKey }),
            object(&*fps),
        ),
        (
            av_key(unsafe { AVVideoProfileLevelKey }),
            object(av_key(unsafe { AVVideoProfileLevelH264MainAutoLevel })),
        ),
    ]);
    let colour = dictionary(&[
        (
            av_key(unsafe { AVVideoColorPrimariesKey }),
            object(av_key(unsafe { AVVideoColorPrimaries_ITU_R_709_2 })),
        ),
        (
            av_key(unsafe { AVVideoTransferFunctionKey }),
            object(av_key(unsafe { AVVideoTransferFunction_ITU_R_709_2 })),
        ),
        (
            av_key(unsafe { AVVideoYCbCrMatrixKey }),
            object(av_key(unsafe { AVVideoYCbCrMatrix_ITU_R_709_2 })),
        ),
    ]);
    let (width, height) = (
        NSNumber::new_f64(layout.width),
        NSNumber::new_f64(layout.height),
    );
    dictionary(&[
        (
            av_key(unsafe { AVVideoCodecKey }),
            object(av_key(unsafe { AVVideoCodecTypeH264 })),
        ),
        (av_key(unsafe { AVVideoWidthKey }), object(&*width)),
        (av_key(unsafe { AVVideoHeightKey }), object(&*height)),
        (
            av_key(unsafe { AVVideoCompressionPropertiesKey }),
            object(&*compression),
        ),
        (
            av_key(unsafe { AVVideoColorPropertiesKey }),
            object(&*colour),
        ),
    ])
}

/// BGRA, IOSurface-backed buffers Core Image can render into on the GPU.
fn buffer_attributes(layout: &ExportLayout) -> Retained<NSDictionary<NSString, AnyObject>> {
    let iosurface: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::new();
    let format = NSNumber::new_u32(kCVPixelFormatType_32BGRA);
    let (width, height) = (
        NSNumber::new_f64(layout.width),
        NSNumber::new_f64(layout.height),
    );
    let yes = NSNumber::new_bool(true);
    dictionary(&[
        (
            av::ns_key(unsafe { kCVPixelBufferPixelFormatTypeKey }),
            object(&*format),
        ),
        (
            av::ns_key(unsafe { kCVPixelBufferWidthKey }),
            object(&*width),
        ),
        (
            av::ns_key(unsafe { kCVPixelBufferHeightKey }),
            object(&*height),
        ),
        (
            av::ns_key(unsafe { kCVPixelBufferIOSurfacePropertiesKey }),
            object(&*iosurface),
        ),
        (
            av::ns_key(unsafe { kCVPixelBufferMetalCompatibilityKey }),
            object(&*yes),
        ),
    ])
}

fn av_key(key: Option<&'static NSString>) -> &'static NSString {
    key.expect("AVFoundation defines its settings keys")
}

/// Any Objective-C or CoreFoundation object as a dictionary value.
fn object<T: ?Sized>(object: &T) -> &AnyObject {
    // SAFETY: only called with Objective-C objects and CoreFoundation objects, which are
    // Objective-C objects at run time (and what NSDictionary stores for them).
    unsafe { &*(object as *const T as *const AnyObject) }
}

fn dictionary(entries: &[(&NSString, &AnyObject)]) -> Retained<NSDictionary<NSString, AnyObject>> {
    let keys: Vec<&NSString> = entries.iter().map(|(key, _)| *key).collect();
    let values: Vec<&AnyObject> = entries.iter().map(|(_, value)| *value).collect();
    NSDictionary::from_slices(&keys, &values)
}

fn cg_rect(rect: Rect) -> CGRect {
    CGRect {
        origin: CGPoint {
            x: rect.x,
            y: rect.y,
        },
        size: CGSize {
            width: rect.width,
            height: rect.height,
        },
    }
}

fn cg_affine([a, b, c, d, tx, ty]: Affine) -> CGAffineTransform {
    CGAffineTransform { a, b, c, d, tx, ty }
}

struct Running {
    shared: Arc<Shared>,
    frame_count: u64,
    done: mpsc::Receiver<Result<(), ExportError>>,
}

impl Encoding for Running {
    fn progress(&self) -> f32 {
        self.shared.frames.load(Ordering::SeqCst) as f32 / self.frame_count.max(1) as f32
    }

    fn outcome(&mut self) -> Option<Result<(), ExportError>> {
        match self.done.try_recv() {
            Ok(outcome) => Some(outcome),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err(failure("the export worker stopped")))
            }
        }
    }

    fn cancel(&mut self) {
        self.shared.cancel.store(true, Ordering::SeqCst);
    }

    fn abandon(&mut self, _output: &Path) {
        // The worker removes the partial file once its writer stopped.
        self.cancel();
    }
}
