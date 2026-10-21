//! The composite MP4 export (DESIGN.md "Export frame", spec §3.5–§3.8).
//!
//! One `AVMutableComposition` with a video track per camera (clips on the wall-clock event timeline)
//! goes through an `AVMutableVideoComposition` that contain-fits every camera into its slot, and an
//! `AVVideoCompositionCoreAnimationTool` that lays one static overlay image on top (floor, rounded
//! holes, wedges, car, labels). Missing tiles (`記録なし`) sit between the video and the overlay and
//! are switched on only for the ranges a camera has no video, with discrete opacity keyframes, so no
//! layer is redrawn per frame. `AVAssetExportSession` encodes H.264 1280 × 720 at 30 fps.

mod plan;

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

use block2::RcBlock;
use objc2::{
    AnyThread, MainThreadMarker, Message,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
};
use objc2_av_foundation::{
    AVAssetExportPreset1280x720, AVAssetExportSession, AVAssetExportSessionStatus, AVAssetTrack,
    AVCoreAnimationBeginTimeAtZero, AVFileTypeMPEG4, AVMutableComposition,
    AVMutableCompositionTrack, AVMutableVideoComposition, AVMutableVideoCompositionInstruction,
    AVMutableVideoCompositionLayerInstruction, AVURLAsset, AVVideoCompositionCoreAnimationTool,
    AVVideoCompositionInstructionProtocol, AVVideoCompositionLayerInstruction,
};
use objc2_core_foundation::{CGAffineTransform, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGColor, CGImage};
use objc2_core_media::CMTime;
use objc2_foundation::{NSArray, NSNumber, NSString};
use objc2_quartz_core::{
    CAKeyframeAnimation, CALayer, CAMediaTiming, kCAAnimationDiscrete, kCAFillModeBoth,
};

pub(crate) use plan::{ExportPlan, MissingTile, plan_export};

use crate::{
    av::{self, SendBox},
    input::{CameraId, CameraMap, EventInput},
    layout::{ExportLayout, Rect, fit_transform},
    overlay::{self, Drawn, OverlayError, VEHICLE_PNG},
    probe::probe_event_blocking,
    status::MediaError,
    timeline::plan_event_timeline,
};

/// Why an export did not produce a file (spec §3.7). [`ExportError::message`] is the UI copy.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportError {
    /// The car image could not be loaded.
    VehicleImage,
    /// Composition, encoder or writer failure (with the AVFoundation error when there is one).
    RecordingFailed(Option<MediaError>),
    /// The event has no clip that can be shown.
    NoPlayableMedia,
    /// [`ExportJob::cancel`] was called; never shown as an error.
    Aborted,
}

impl ExportError {
    /// The contract error code (`recording-failed`, `no-playable-media`, `aborted`).
    pub fn code(&self) -> &'static str {
        match self {
            ExportError::VehicleImage | ExportError::RecordingFailed(_) => "recording-failed",
            ExportError::NoPlayableMedia => "no-playable-media",
            ExportError::Aborted => "aborted",
        }
    }

    /// The Japanese message of spec §5.7.
    pub fn message(&self) -> &'static str {
        match self {
            ExportError::VehicleImage => "車両イメージを読み込めませんでした。",
            ExportError::RecordingFailed(_) => "動画の録画に失敗しました。",
            ExportError::NoPlayableMedia => "再生可能な動画がありません。",
            ExportError::Aborted => "動画の書き出しをキャンセルしました。",
        }
    }
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExportError::RecordingFailed(Some(error)) => write!(f, "{} ({error})", self.message()),
            _ => f.write_str(self.message()),
        }
    }
}

impl std::error::Error for ExportError {}

/// Where an export stands; see [`ExportJob::poll`].
#[derive(Debug, Clone, PartialEq)]
pub enum ExportState {
    /// Probing files and drawing the overlay.
    Preparing,
    /// Encoding; `progress` runs from 0 to 1 over the whole event.
    Exporting {
        /// 0…1.
        progress: f32,
    },
    /// The MP4 is complete at `path`.
    Finished {
        /// The output file.
        path: PathBuf,
        /// Duration of the exported video in seconds.
        duration: f64,
        /// Frames in the exported video.
        frames: u64,
    },
    /// The export stopped (partial output is deleted).
    Failed(ExportError),
}

impl ExportState {
    /// True for `Finished` and `Failed`.
    pub fn is_done(&self) -> bool {
        matches!(self, ExportState::Finished { .. } | ExportState::Failed(_))
    }
}

/// Builds composite exports. Holds the layout (from `packages/contract`) and the car image.
#[derive(Debug, Clone)]
pub struct CompositeExporter {
    layout: ExportLayout,
    vehicle_png: Cow<'static, [u8]>,
}

impl Default for CompositeExporter {
    fn default() -> Self {
        CompositeExporter::new()
    }
}

/// One camera's composition track and the source track (with its start on the event axis) of each
/// of its clips.
struct CameraTrack {
    track: Retained<AVMutableCompositionTrack>,
    sources: Vec<(f64, Retained<AVAssetTrack>)>,
}

/// Everything the worker thread prepares off the main thread.
struct Prepared {
    plan: ExportPlan,
    composition: Retained<AVMutableComposition>,
    /// Composition track per camera with video.
    tracks: CameraMap<Option<CameraTrack>>,
    overlay: Drawn,
    missing: CameraMap<Option<Drawn>>,
    _assets: Vec<Retained<AVURLAsset>>,
}

enum Stage {
    Preparing(mpsc::Receiver<Result<SendBox<Prepared>, ExportError>>),
    Exporting {
        session: Retained<AVAssetExportSession>,
        done: Arc<AtomicBool>,
        plan: Box<ExportPlan>,
    },
    Done,
}

/// A running export. Main thread only; call [`ExportJob::poll`] regularly (e.g. once per frame).
pub struct ExportJob {
    output: PathBuf,
    layout: ExportLayout,
    stage: Stage,
    state: ExportState,
    cancel: Arc<AtomicBool>,
    on_progress: Box<dyn FnMut(f32)>,
    reported: f32,
    _mtm: MainThreadMarker,
}

impl CompositeExporter {
    /// The contract layout with the bundled car image.
    pub fn new() -> CompositeExporter {
        CompositeExporter {
            layout: ExportLayout::contract(),
            vehicle_png: Cow::Borrowed(VEHICLE_PNG),
        }
    }

    /// Uses another car image (PNG bytes).
    pub fn with_vehicle_png(mut self, png: Vec<u8>) -> CompositeExporter {
        self.vehicle_png = Cow::Owned(png);
        self
    }

    /// The export frame layout.
    pub fn layout(&self) -> &ExportLayout {
        &self.layout
    }

    /// Starts exporting `event` to `output` (an existing file there is replaced). Returns
    /// immediately; probing and drawing run on a worker thread. `on_progress` receives 0 when encoding
    /// starts, increasing values while it runs and 1 once the file is complete (called from
    /// [`ExportJob::poll`]).
    pub fn start(
        &self,
        event: &EventInput,
        output: &Path,
        on_progress: impl FnMut(f32) + 'static,
        mtm: MainThreadMarker,
    ) -> ExportJob {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let (event, layout, vehicle, worker_cancel) = (
            event.clone(),
            self.layout.clone(),
            self.vehicle_png.clone().into_owned(),
            cancel.clone(),
        );
        thread::spawn(move || {
            let _ = tx.send(prepare(&event, &layout, &vehicle, &worker_cancel).map(SendBox));
        });
        ExportJob {
            output: output.to_path_buf(),
            layout: self.layout.clone(),
            stage: Stage::Preparing(rx),
            state: ExportState::Preparing,
            cancel,
            on_progress: Box::new(on_progress),
            reported: -1.0,
            _mtm: mtm,
        }
    }
}

impl ExportJob {
    /// Advances the export and returns its state.
    pub fn poll(&mut self) -> &ExportState {
        match &self.stage {
            Stage::Preparing(rx) => match rx.try_recv() {
                Ok(Ok(SendBox(prepared))) if !self.cancel.load(Ordering::SeqCst) => {
                    self.begin(prepared)
                }
                Ok(Err(error)) => self.finish(ExportState::Failed(error)),
                Ok(Ok(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                    self.finish(ExportState::Failed(ExportError::Aborted))
                }
                Err(mpsc::TryRecvError::Empty) => {}
            },
            Stage::Exporting {
                session,
                done,
                plan,
            } => {
                if done.load(Ordering::SeqCst) {
                    let state = match unsafe { session.status() } {
                        AVAssetExportSessionStatus::Completed => ExportState::Finished {
                            path: self.output.clone(),
                            duration: plan.duration,
                            frames: plan.frame_count,
                        },
                        AVAssetExportSessionStatus::Cancelled => {
                            ExportState::Failed(ExportError::Aborted)
                        }
                        _ => {
                            let error = unsafe { session.error() }.map(|e| av::media_error(&e));
                            ExportState::Failed(ExportError::RecordingFailed(error))
                        }
                    };
                    self.finish(state);
                } else {
                    let progress = unsafe { session.progress() }.clamp(0.0, 0.999);
                    self.state = ExportState::Exporting { progress };
                    self.report(progress);
                }
            }
            Stage::Done => {}
        }
        &self.state
    }

    /// The last polled state.
    pub fn state(&self) -> &ExportState {
        &self.state
    }

    /// Stops the export; the next [`ExportJob::poll`] reports [`ExportError::Aborted`] once
    /// AVFoundation stopped, and any partial file is deleted.
    pub fn cancel(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        match &self.stage {
            Stage::Exporting { session, .. } => unsafe { session.cancelExport() },
            Stage::Preparing(_) => self.finish(ExportState::Failed(ExportError::Aborted)),
            Stage::Done => {}
        }
    }

    fn report(&mut self, progress: f32) {
        if progress > self.reported {
            self.reported = progress;
            (self.on_progress)(progress);
        }
    }

    fn finish(&mut self, state: ExportState) {
        if let ExportState::Failed(_) = state {
            let _ = std::fs::remove_file(&self.output);
        } else {
            self.report(1.0);
        }
        self.stage = Stage::Done;
        self.state = state;
    }

    fn begin(&mut self, prepared: Prepared) {
        match start_session(&prepared, &self.layout, &self.output) {
            Ok((session, done)) => {
                self.stage = Stage::Exporting {
                    session,
                    done,
                    plan: Box::new(prepared.plan),
                };
                self.state = ExportState::Exporting { progress: 0.0 };
                self.report(0.0);
            }
            Err(error) => self.finish(ExportState::Failed(error)),
        }
    }
}

impl Drop for ExportJob {
    fn drop(&mut self) {
        if let Stage::Exporting { session, .. } = &self.stage {
            unsafe { session.cancelExport() };
            let _ = std::fs::remove_file(&self.output);
        }
        self.cancel.store(true, Ordering::SeqCst);
    }
}

/// Worker thread: probe, plan, compose and draw the static images.
fn prepare(
    event: &EventInput,
    layout: &ExportLayout,
    vehicle_png: &[u8],
    cancel: &AtomicBool,
) -> Result<Prepared, ExportError> {
    let probed = probe_event_blocking(event, cancel).ok_or(ExportError::Aborted)?;
    let timeline = plan_event_timeline(&probed.segments);
    let plan = plan_export(&timeline, layout).ok_or(ExportError::NoPlayableMedia)?;
    let holes = plan.cameras.map(|_, camera| camera.hole);
    let overlay =
        overlay::draw_overlay(layout, &holes, vehicle_png).map_err(|error| match error {
            OverlayError::VehicleImage => ExportError::VehicleImage,
            OverlayError::Graphics => {
                ExportError::RecordingFailed(Some(av::local_error("could not draw the overlay")))
            }
        })?;
    let mut missing = CameraMap::from_fn(|_| None);
    for camera in CameraId::ALL {
        if plan.cameras[camera].missing != MissingTile::Never {
            let hole = plan.cameras[camera].hole;
            let tile =
                overlay::draw_missing_tile(layout, (hole.width, hole.height)).map_err(|_| {
                    ExportError::RecordingFailed(Some(av::local_error(
                        "could not draw a missing tile",
                    )))
                })?;
            missing[camera] = Some(tile);
        }
    }
    let composition = unsafe { AVMutableComposition::composition() };
    let mut tracks = CameraMap::from_fn(|_| None);
    for camera in CameraId::ALL {
        let camera_timeline = &timeline.cameras[camera];
        if !camera_timeline.has_video() {
            continue;
        }
        let sources: Vec<(f64, Retained<AVAssetTrack>)> = camera_timeline
            .clips
            .iter()
            .map(|clip| {
                let track = probed
                    .track(clip.segment, camera)
                    .expect("placed clips have a loaded track");
                (clip.start, track.retain())
            })
            .collect();
        let track =
            av::add_video_track(&composition).map_err(|e| ExportError::RecordingFailed(Some(e)))?;
        let refs: Vec<&AVAssetTrack> = sources.iter().map(|(_, track)| &**track).collect();
        av::insert_clips(&track, camera_timeline, &refs, timeline.duration)
            .map_err(|e| ExportError::RecordingFailed(Some(e)))?;
        tracks[camera] = Some(CameraTrack { track, sources });
    }
    if cancel.load(Ordering::SeqCst) {
        return Err(ExportError::Aborted);
    }
    let assets = probed
        .files
        .iter()
        .flat_map(|cameras| {
            cameras
                .iter()
                .filter_map(|(_, file)| file.as_ref()?.media.as_ref())
        })
        .map(|(asset, _)| asset.clone())
        .collect();
    Ok(Prepared {
        plan,
        composition,
        tracks,
        overlay,
        missing,
        _assets: assets,
    })
}

fn cg_rect_bottom_left(rect: Rect, height: f64) -> CGRect {
    // CALayer geometry on macOS has its origin at the bottom-left.
    CGRect {
        origin: CGPoint {
            x: rect.x,
            y: height - rect.y - rect.height,
        },
        size: CGSize {
            width: rect.width,
            height: rect.height,
        },
    }
}

fn image_layer(image: &CGImage, frame: CGRect) -> Retained<CALayer> {
    let layer = CALayer::new();
    layer.setFrame(frame);
    // SAFETY: CALayer.contents accepts a CGImage.
    unsafe { layer.setContents(Some(&*(image as *const CGImage as *const AnyObject))) };
    layer
}

fn opacity_animation(plan: &plan::Keyframes, duration: f64) -> Retained<CAKeyframeAnimation> {
    let animation = CAKeyframeAnimation::animationWithKeyPath(Some(&NSString::from_str("opacity")));
    let values: Vec<Retained<NSNumber>> =
        plan.values.iter().map(|v| NSNumber::new_f64(*v)).collect();
    let key_times: Vec<Retained<NSNumber>> = plan
        .key_times
        .iter()
        .map(|t| NSNumber::new_f64(*t))
        .collect();
    unsafe {
        let values = NSArray::from_retained_slice(&values);
        // SAFETY: an NSArray of NSNumbers is an NSArray of objects; the element type is only static.
        let values = &*(Retained::as_ptr(&values) as *const NSArray);
        animation.setValues(Some(values));
        animation.setKeyTimes(Some(&NSArray::from_retained_slice(&key_times)));
        animation.setCalculationMode(kCAAnimationDiscrete);
        // AVFoundation treats a beginTime of 0 as "now"; this constant means the start of the video.
        animation.setBeginTime(AVCoreAnimationBeginTimeAtZero);
        animation.setFillMode(kCAFillModeBoth);
    }
    animation.setDuration(duration);
    animation.setRemovedOnCompletion(false);
    animation
}

/// Main thread: video composition, CoreAnimation layers and the export session.
fn start_session(
    prepared: &Prepared,
    layout: &ExportLayout,
    output: &Path,
) -> Result<(Retained<AVAssetExportSession>, Arc<AtomicBool>), ExportError> {
    let failed = |what: &str| ExportError::RecordingFailed(Some(av::local_error(what)));
    let plan = &prepared.plan;
    let (width, height) = (layout.width, layout.height);

    let video_composition = unsafe { AVMutableVideoComposition::videoComposition() };
    unsafe {
        video_composition.setRenderSize(CGSize { width, height });
        video_composition.setFrameDuration(CMTime::new(1, layout.fps as i32));
    }
    let instruction =
        unsafe { AVMutableVideoCompositionInstruction::videoCompositionInstruction() };
    let floor = CGColor::new_srgb(layout.floor.r, layout.floor.g, layout.floor.b, 1.0);
    unsafe {
        instruction.setTimeRange(av::range(0.0, plan.duration));
        instruction.setBackgroundColor(Some(&floor));
    }
    let mut layer_instructions: Vec<Retained<AVVideoCompositionLayerInstruction>> = Vec::new();
    for camera in CameraId::ALL {
        let Some(CameraTrack { track, sources }) = &prepared.tracks[camera] else {
            continue;
        };
        let instruction = unsafe {
            AVMutableVideoCompositionLayerInstruction::videoCompositionLayerInstructionWithAssetTrack(track)
        };
        for (index, (start, source)) in sources.iter().enumerate() {
            let (natural, preferred) =
                unsafe { (source.naturalSize(), source.preferredTransform()) };
            let [a, b, c, d, tx, ty] = fit_transform(
                (natural.width, natural.height),
                av::affine_parts(preferred),
                layout.slots[camera],
            );
            let time = if index == 0 { 0.0 } else { *start };
            unsafe {
                instruction
                    .setTransform_atTime(CGAffineTransform { a, b, c, d, tx, ty }, av::cm(time))
            };
        }
        layer_instructions.push(Retained::into_super(instruction));
    }
    unsafe {
        instruction.setLayerInstructions(&NSArray::from_retained_slice(&layer_instructions));
        let instruction: Retained<ProtocolObject<dyn AVVideoCompositionInstructionProtocol>> =
            ProtocolObject::from_retained(instruction);
        video_composition.setInstructions(&NSArray::from_retained_slice(&[instruction]));
    }

    // Layers, bottom to top: video, missing tiles, overlay.
    let full = CGRect {
        origin: CGPoint { x: 0.0, y: 0.0 },
        size: CGSize { width, height },
    };
    let parent = CALayer::new();
    parent.setFrame(full);
    let video = CALayer::new();
    video.setFrame(full);
    parent.addSublayer(&video);
    for camera in CameraId::ALL {
        let Some(tile) = &prepared.missing[camera] else {
            continue;
        };
        let layer = image_layer(
            &tile.image,
            cg_rect_bottom_left(plan.cameras[camera].hole, height),
        );
        match &plan.cameras[camera].missing {
            MissingTile::Never => continue,
            MissingTile::Always => layer.setOpacity(1.0),
            MissingTile::During(keyframes) => {
                layer.setOpacity(0.0);
                layer.addAnimation_forKey(
                    &opacity_animation(keyframes, plan.duration),
                    Some(&NSString::from_str("missing")),
                );
            }
        }
        parent.addSublayer(&layer);
    }
    parent.addSublayer(&image_layer(&prepared.overlay.image, full));
    let tool = unsafe {
        AVVideoCompositionCoreAnimationTool::videoCompositionCoreAnimationToolWithPostProcessingAsVideoLayer_inLayer(&video, &parent)
    };
    unsafe { video_composition.setAnimationTool(Some(&tool)) };

    let _ = std::fs::remove_file(output);
    let preset = unsafe { AVAssetExportPreset1280x720 };
    let session = unsafe {
        AVAssetExportSession::initWithAsset_presetName(
            AVAssetExportSession::alloc(),
            &prepared.composition,
            preset,
        )
    }
    .ok_or_else(|| failed("could not create the export session"))?;
    unsafe {
        session.setOutputURL(Some(&av::file_url(output)));
        session.setOutputFileType(AVFileTypeMPEG4);
        session.setShouldOptimizeForNetworkUse(true);
        session.setVideoComposition(Some(&video_composition));
    }
    let done = Arc::new(AtomicBool::new(false));
    let flag = done.clone();
    let handler = RcBlock::new(move || flag.store(true, Ordering::SeqCst));
    unsafe { session.exportAsynchronouslyWithCompletionHandler(&handler) };
    Ok((session, done))
}
