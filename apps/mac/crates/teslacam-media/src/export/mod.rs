//! The composite MP4 export (DESIGN.md "Export frame", spec §3.5–§3.8).
//!
//! Every output frame is the same picture: the floor, every camera contain-fitted into its slot, the
//! missing tile (`記録なし`) over a camera's window while it has no video, and on top one static
//! overlay image (floor, rounded holes, wedges, car, labels) drawn once per export by
//! [`crate::overlay`]. H.264, 1280 × 720, 30 fps; output frame `n` shows each camera's latest frame at
//! or before `n / 30` s on the wall-clock event timeline. Two pipelines draw it:
//!
//! - [`ExportBackend::ReaderWriter`], the default (`writer`): one `AVAssetReader` per camera decodes
//!   in hardware on its own thread, Core Image composes every frame on the GPU and `AVAssetWriter`
//!   encodes it in hardware, all off the main thread.
//! - [`ExportBackend::ExportSession`] (`session`): `AVAssetExportSession` with an
//!   `AVMutableVideoComposition` and Core Animation layers, the pipeline of v0.1.0, kept to compare
//!   against.
//!
//! Both prepare on a worker thread (probe, plan, draw the static images) and touch the destination
//! file only once the main thread's [`ExportJob::poll`] has seen the preparation finish and the job
//! was not cancelled.

mod frames;
mod plan;
mod session;
mod writer;

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

use objc2::{MainThreadMarker, Message, rc::Retained};
use objc2_av_foundation::AVAssetTrack;

pub(crate) use plan::{ExportPlan, MissingTile, plan_export};

use crate::{
    av::{self, SendBox},
    input::{CameraId, CameraMap, EventInput},
    layout::ExportLayout,
    overlay::{self, Drawn, OverlayError, VEHICLE_PNG},
    probe::{ProbedEvent, probe_event_blocking},
    status::MediaError,
    timeline::{EventTimeline, plan_event_timeline},
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

/// Which pipeline encodes the export; see the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExportBackend {
    /// `AVAssetReader` per camera → Core Image → `AVAssetWriter`, on worker threads (the default).
    #[default]
    ReaderWriter,
    /// `AVAssetExportSession` with a video composition and Core Animation (v0.1.0).
    ExportSession,
}

/// Builds composite exports. Holds the layout (from `packages/contract`), the car image and the
/// pipeline.
#[derive(Debug, Clone)]
pub struct CompositeExporter {
    layout: ExportLayout,
    vehicle_png: Cow<'static, [u8]>,
    backend: ExportBackend,
}

impl Default for CompositeExporter {
    fn default() -> Self {
        CompositeExporter::new()
    }
}

/// What both pipelines start from, prepared on the worker thread.
pub(crate) struct Prepared {
    /// The loaded assets and tracks (kept alive for the whole export).
    probed: ProbedEvent,
    timeline: EventTimeline,
    plan: ExportPlan,
    overlay: Drawn,
    /// The missing tile of each camera that ever shows it.
    missing: CameraMap<Option<Drawn>>,
}

impl Prepared {
    /// The source track of each clip of `camera`, with the clip's start on the event axis.
    fn sources(&self, camera: CameraId) -> Vec<(f64, Retained<AVAssetTrack>)> {
        self.timeline.cameras[camera]
            .clips
            .iter()
            .map(|clip| {
                let track = self
                    .probed
                    .track(clip.segment, camera)
                    .expect("placed clips have a loaded track");
                (clip.start, track.retain())
            })
            .collect()
    }
}

/// A prepared export, waiting for the main thread to let it write the output file.
struct Ready {
    plan: ExportPlan,
    pipeline: ReadyPipeline,
}

enum ReadyPipeline {
    Session(Box<session::Composed>),
    Writer(writer::Pending),
}

/// A running encoder, owned by the [`ExportJob`] on the main thread.
trait Encoding {
    /// 0…1 while encoding.
    fn progress(&self) -> f32;
    /// `Some` once the encoder stopped: `Ok` when the file is complete.
    fn outcome(&mut self) -> Option<Result<(), ExportError>>;
    /// Asks the encoder to stop; [`Encoding::outcome`] then reports [`ExportError::Aborted`].
    fn cancel(&mut self);
    /// The job is dropped while encoding: stop without waiting, and remove the partial `output`.
    fn abandon(&mut self, output: &Path);
}

enum Stage {
    Preparing(mpsc::Receiver<Result<SendBox<Ready>, ExportError>>),
    Exporting {
        encoding: Box<dyn Encoding>,
        plan: Box<ExportPlan>,
    },
    Done,
}

/// A running export. Main thread only; call [`ExportJob::poll`] regularly (e.g. once per frame).
///
/// Prefer [`ExportJob::cancel`] followed by polling until done: it waits for AVFoundation to stop
/// before deleting the partial file. Dropping a running job also cancels it, but cannot wait, so the
/// partial file is removed on a best-effort basis.
pub struct ExportJob {
    output: PathBuf,
    layout: ExportLayout,
    stage: Stage,
    state: ExportState,
    cancel: Arc<AtomicBool>,
    on_progress: Box<dyn FnMut(f32)>,
    reported: f32,
    /// Whether the exporter has started writing `output` (it replaces an existing file only then).
    wrote_output: bool,
    _mtm: MainThreadMarker,
}

impl CompositeExporter {
    /// The contract layout with the bundled car image and the default pipeline.
    pub fn new() -> CompositeExporter {
        CompositeExporter {
            layout: ExportLayout::contract(),
            vehicle_png: Cow::Borrowed(VEHICLE_PNG),
            backend: ExportBackend::default(),
        }
    }

    /// Uses another car image (PNG bytes).
    pub fn with_vehicle_png(mut self, png: Vec<u8>) -> CompositeExporter {
        self.vehicle_png = Cow::Owned(png);
        self
    }

    /// Uses another pipeline.
    pub fn with_backend(mut self, backend: ExportBackend) -> CompositeExporter {
        self.backend = backend;
        self
    }

    /// The export frame layout.
    pub fn layout(&self) -> &ExportLayout {
        &self.layout
    }

    /// The pipeline exports run through.
    pub fn backend(&self) -> ExportBackend {
        self.backend
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
        let (event, layout, vehicle, backend, path, worker_cancel) = (
            event.clone(),
            self.layout.clone(),
            self.vehicle_png.clone().into_owned(),
            self.backend,
            output.to_path_buf(),
            cancel.clone(),
        );
        thread::spawn(move || {
            av::run_as_user_initiated();
            let prepared = match prepare(&event, &layout, &vehicle, &worker_cancel) {
                Ok(prepared) => prepared,
                Err(error) => {
                    let _ = tx.send(Err(error));
                    return;
                }
            };
            let plan = prepared.plan.clone();
            match backend {
                ExportBackend::ExportSession => {
                    let ready = session::compose(prepared).map(|composed| {
                        SendBox(Ready {
                            plan,
                            pipeline: ReadyPipeline::Session(Box::new(composed)),
                        })
                    });
                    let _ = tx.send(ready);
                }
                ExportBackend::ReaderWriter => {
                    match writer::compose(prepared, layout, path, worker_cancel) {
                        Ok((pending, encoder)) => {
                            let ready = Ready {
                                plan,
                                pipeline: ReadyPipeline::Writer(pending),
                            };
                            if tx.send(Ok(SendBox(ready))).is_ok() {
                                encoder.run();
                            }
                        }
                        Err(error) => {
                            let _ = tx.send(Err(error));
                        }
                    }
                }
            }
        });
        ExportJob {
            output: output.to_path_buf(),
            layout: self.layout.clone(),
            stage: Stage::Preparing(rx),
            state: ExportState::Preparing,
            cancel,
            on_progress: Box::new(on_progress),
            reported: -1.0,
            wrote_output: false,
            _mtm: mtm,
        }
    }
}

impl ExportJob {
    /// Advances the export and returns its state.
    pub fn poll(&mut self) -> &ExportState {
        match &mut self.stage {
            Stage::Preparing(rx) => match rx.try_recv() {
                Ok(Ok(SendBox(ready))) if !self.cancel.load(Ordering::SeqCst) => self.begin(ready),
                Ok(Err(error)) => self.finish(ExportState::Failed(error)),
                Ok(Ok(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                    self.finish(ExportState::Failed(ExportError::Aborted))
                }
                Err(mpsc::TryRecvError::Empty) => {}
            },
            Stage::Exporting { encoding, plan } => match encoding.outcome() {
                Some(Ok(())) => {
                    let state = ExportState::Finished {
                        path: self.output.clone(),
                        duration: plan.duration,
                        frames: plan.frame_count,
                    };
                    self.finish(state);
                }
                Some(Err(error)) => self.finish(ExportState::Failed(error)),
                None => {
                    let progress = encoding.progress().clamp(0.0, 0.999);
                    self.state = ExportState::Exporting { progress };
                    self.report(progress);
                }
            },
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
        match &mut self.stage {
            Stage::Exporting { encoding, .. } => encoding.cancel(),
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
            // Only output this export wrote is removed: a file that was at the destination before
            // encoding started (the save panel only confirmed replacing it) stays.
            if self.wrote_output {
                let _ = std::fs::remove_file(&self.output);
            }
        } else {
            self.report(1.0);
        }
        self.stage = Stage::Done;
        self.state = state;
    }

    fn begin(&mut self, ready: Ready) {
        self.wrote_output = true;
        let started = match ready.pipeline {
            ReadyPipeline::Session(composed) => {
                session::start(*composed, &self.layout, &self.output)
            }
            ReadyPipeline::Writer(pending) => Ok(pending.start()),
        };
        match started {
            Ok(encoding) => {
                self.stage = Stage::Exporting {
                    encoding,
                    plan: Box::new(ready.plan),
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
        self.cancel.store(true, Ordering::SeqCst);
        if let Stage::Exporting { encoding, .. } = &mut self.stage {
            encoding.abandon(&self.output);
        }
    }
}

/// Worker thread: probe, plan and draw the static images.
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
            OverlayError::Graphics => failure("could not draw the overlay"),
        })?;
    let mut missing = CameraMap::from_fn(|_| None);
    for camera in CameraId::ALL {
        if plan.cameras[camera].missing != MissingTile::Never {
            let hole = plan.cameras[camera].hole;
            let tile = overlay::draw_missing_tile(layout, (hole.width, hole.height))
                .map_err(|_| failure("could not draw a missing tile"))?;
            missing[camera] = Some(tile);
        }
    }
    if cancel.load(Ordering::SeqCst) {
        return Err(ExportError::Aborted);
    }
    Ok(Prepared {
        probed,
        timeline,
        plan,
        overlay,
        missing,
    })
}

/// A recording failure that did not come from AVFoundation.
fn failure(what: &str) -> ExportError {
    ExportError::RecordingFailed(Some(av::local_error(what)))
}

/// A recording failure with AVFoundation's error.
fn media_failure(error: MediaError) -> ExportError {
    ExportError::RecordingFailed(Some(error))
}
