//! Synchronized six-camera playback of one event.

use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
};

use objc2::{MainThreadMarker, rc::Retained};
use objc2_av_foundation::{AVMutableComposition, AVURLAsset};

use crate::{
    av::{self, SendBox},
    engine::{Engine, Tile, TileState},
    frame::VideoFrame,
    input::{CameraId, CameraMap, EventInput},
    probe::probe_event_blocking,
    status::{CameraStatus, FileFault, classify_error},
    sync::{Command, Notice, SyncMachine},
    timeline::{Coverage, EventTimeline, SegmentProbe, plan_event_timeline},
};

/// Something that happened since the previous [`EventPlayback::tick`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackEvent {
    /// Probing and composing finished; the event lasts `duration` seconds (0 when nothing can be
    /// played: every camera then reports a terminal status).
    Loaded {
        /// Event duration in seconds on the wall-clock timeline.
        duration: f64,
    },
    /// Playback reached the end of the event and paused there.
    Ended,
}

/// Lowest and highest accepted playback rates.
pub const RATE_RANGE: (f32, f32) = (0.25, 4.0);

type Compositions = CameraMap<Option<Result<Retained<AVMutableComposition>, FileFault>>>;

struct Composed {
    segments: Vec<SegmentProbe>,
    timeline: EventTimeline,
    compositions: Compositions,
    assets: Vec<Retained<AVURLAsset>>,
}

struct Loaded {
    segments: Vec<SegmentProbe>,
    timeline: EventTimeline,
    /// Engine tile index per camera, or the reason the camera's composition failed.
    tiles: CameraMap<Option<Result<usize, FileFault>>>,
    /// Source assets stay alive as long as the compositions that reference them.
    _assets: Vec<Retained<AVURLAsset>>,
}

/// Six `AVPlayer`s (one per camera) that play one event in sync on the wall-clock event timeline.
///
/// # Threading
///
/// Main thread only (`AVPlayer` is main-thread-only; construction takes a [`MainThreadMarker`]).
/// Opening returns immediately: files are probed and composed on a worker thread, and players are
/// created on the main thread by a later [`EventPlayback::tick`]. Nothing here blocks or pumps the
/// run loop; AVFoundation completion handlers are collected and applied in `tick`, so call `tick`
/// once per rendered frame (and at least every few hundred milliseconds while a command is in
/// flight).
///
/// # Time
///
/// `now` arguments are host seconds ([`crate::host_now`]). Positions are event seconds: segment `i`
/// starts at its wall-clock offset from segment 0, and each clip is truncated at the next segment.
/// Commands issued before loading finished (an initial seek, autoplay) are remembered and applied
/// once the players are ready.
pub struct EventPlayback {
    event: EventInput,
    engine: Engine,
    loader: Option<mpsc::Receiver<SendBox<Composed>>>,
    loaded: Option<Loaded>,
    cancel: Arc<AtomicBool>,
    mtm: MainThreadMarker,
}

impl EventPlayback {
    /// Starts loading `event`. Returns immediately; see the type docs.
    pub fn open(event: &EventInput, mtm: MainThreadMarker) -> EventPlayback {
        let (tx, rx) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let worker_event = event.clone();
        let worker_cancel = cancel.clone();
        thread::spawn(move || {
            if let Some(composed) = compose_event(&worker_event, &worker_cancel) {
                let _ = tx.send(SendBox(composed));
            }
        });
        EventPlayback {
            event: event.clone(),
            engine: Engine::new(SyncMachine::new()),
            loader: Some(rx),
            loaded: None,
            cancel,
            mtm,
        }
    }

    /// Applies finished background work and advances seeks / starts. Call once per frame.
    pub fn tick(&mut self, now: f64) -> Vec<PlaybackEvent> {
        let mut events = Vec::new();
        if let Some(composed) = self.loader.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.loader = None;
            let duration = composed.0.timeline.duration;
            self.install(composed.0);
            events.push(PlaybackEvent::Loaded { duration });
        }
        let Some(loaded) = &self.loaded else {
            return events;
        };
        let timeline = &loaded.timeline;
        let tiles = &loaded.tiles;
        let covered = |tile: usize, t: f64| {
            CameraId::ALL.iter().any(|camera| {
                tiles[*camera] == Some(Ok(tile))
                    && matches!(
                        timeline.cameras[*camera].coverage_at(t),
                        Some(Coverage::Video { .. })
                    )
            })
        };
        if self.engine.tick(now, covered) == Some(Notice::Ended) {
            events.push(PlaybackEvent::Ended);
        }
        events
    }

    fn install(&mut self, composed: Composed) {
        let mut players = Vec::new();
        let tiles = composed
            .compositions
            .map(|_, composition| match composition {
                None => None,
                Some(Err(fault)) => Some(Err(*fault)),
                Some(Ok(composition)) => {
                    players.push(Tile::new(composition, self.engine.clock(), self.mtm));
                    Some(Ok(players.len() - 1))
                }
            });
        self.engine.attach(players, composed.timeline.duration);
        self.loaded = Some(Loaded {
            segments: composed.segments,
            timeline: composed.timeline,
            tiles,
            _assets: composed.assets,
        });
    }

    /// Requests playback (from 0 when at the end). Takes effect once every ready player is in place.
    pub fn play(&mut self, now: f64) {
        self.engine.command(Command::Play, now);
    }

    /// Pauses every camera on the same frame.
    pub fn pause(&mut self, now: f64) {
        self.engine.command(Command::Pause, now);
    }

    /// Moves every camera exactly to event time `seconds` (clamped), keeping the play intent.
    pub fn seek(&mut self, seconds: f64, now: f64) {
        self.engine.command(Command::Seek(seconds), now);
    }

    /// Sets the playback rate, clamped to [`RATE_RANGE`]. The UI offers 0.5, 1, 1.5 and 2.
    pub fn set_rate(&mut self, rate: f32, now: f64) {
        let rate = if rate.is_finite() {
            rate.clamp(RATE_RANGE.0, RATE_RANGE.1)
        } else {
            1.0
        };
        self.engine.command(Command::SetRate(rate), now);
    }

    /// The requested playback rate.
    pub fn rate(&self) -> f32 {
        self.engine.machine().rate()
    }

    /// True while playback is requested (also while a seek or start is still settling).
    pub fn is_playing(&self) -> bool {
        self.engine.machine().wants_play()
    }

    /// True once loaded and no seek or start is in flight (the frames match the position).
    pub fn is_settled(&self) -> bool {
        self.engine.machine().is_settled()
    }

    /// True while the players are actually running on the shared clock.
    pub fn is_running(&self) -> bool {
        self.engine.machine().is_running()
    }

    /// Event time at host time `now`.
    pub fn current_time(&self, now: f64) -> f64 {
        self.engine.machine().position(now)
    }

    /// Event duration once loaded.
    pub fn duration(&self) -> Option<f64> {
        self.loaded.as_ref().map(|loaded| loaded.timeline.duration)
    }

    /// The wall-clock timeline (clips, gaps) once loaded.
    pub fn timeline(&self) -> Option<&EventTimeline> {
        self.loaded.as_ref().map(|loaded| &loaded.timeline)
    }

    /// Per-file probe results (durations, sizes, decodability) once loaded.
    pub fn probe(&self) -> Option<&[SegmentProbe]> {
        self.loaded
            .as_ref()
            .map(|loaded| loaded.segments.as_slice())
    }

    /// What `camera`'s window shows at host time `now`.
    pub fn status(&mut self, camera: CameraId, now: f64) -> CameraStatus {
        let t = self.current_time(now);
        let Some(loaded) = &self.loaded else {
            let segment = segment_index(&self.event, t);
            let has_file = self
                .event
                .segments
                .get(segment)
                .is_some_and(|s| s.files[camera].is_some());
            return if has_file {
                CameraStatus::Loading
            } else {
                CameraStatus::Missing
            };
        };
        match loaded.timeline.cameras[camera].coverage_at(t) {
            None => {
                // Nothing in the event can be played: report what the probe found for this camera.
                let segment = loaded.timeline.segment_at(t);
                match loaded.segments.get(segment).and_then(|s| s.cameras[camera]) {
                    Some(Err(fault)) => fault.into(),
                    _ => CameraStatus::Missing,
                }
            }
            Some(Coverage::Missing) => CameraStatus::Missing,
            Some(Coverage::Fault(fault)) => fault.into(),
            Some(Coverage::Video { .. }) => match loaded.tiles[camera] {
                None => CameraStatus::Missing,
                Some(Err(fault)) => fault.into(),
                Some(Ok(index)) => match self.engine.tile_state(index) {
                    TileState::Loading => CameraStatus::Loading,
                    TileState::Ready => CameraStatus::Ready,
                    TileState::Failed(fault) => fault.into(),
                },
            },
        }
    }

    /// The frame `camera` shows at host time `now`, or `None` when it has no video then (gap,
    /// fault, still loading). While a seek is settling the previous frame is returned unchanged.
    pub fn frame(&mut self, camera: CameraId, now: f64) -> Option<VideoFrame> {
        let t = self.current_time(now);
        let loaded = self.loaded.as_ref()?;
        let Some(Coverage::Video { clip }) = loaded.timeline.cameras[camera].coverage_at(t) else {
            return None;
        };
        let Some(Ok(index)) = loaded.tiles[camera] else {
            return None;
        };
        let info = loaded.timeline.cameras[camera].clips[clip].info;
        let buffer = self.engine.pull(index, now)?.clone();
        Some(VideoFrame {
            buffer,
            display_width: info.display_width,
            display_height: info.display_height,
        })
    }
}

impl Drop for EventPlayback {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

fn segment_index(event: &EventInput, t: f64) -> usize {
    let origin = event.segments.first().map_or(0.0, |s| s.start_seconds);
    event
        .segments
        .partition_point(|s| s.start_seconds - origin <= t)
        .saturating_sub(1)
}

/// Worker thread: probe, plan the timeline, build one composition per camera.
fn compose_event(event: &EventInput, cancel: &AtomicBool) -> Option<Composed> {
    let probed = probe_event_blocking(event, cancel)?;
    let timeline = plan_event_timeline(&probed.segments);
    let compositions = CameraMap::from_fn(|camera| {
        let camera_timeline = &timeline.cameras[camera];
        if !camera_timeline.has_video() {
            return None;
        }
        let sources: Vec<_> = camera_timeline
            .clips
            .iter()
            .map(|clip| {
                probed
                    .track(clip.segment, camera)
                    .expect("placed clips have a loaded track")
            })
            .collect();
        Some(
            av::camera_composition(camera_timeline, &sources, timeline.duration)
                .map_err(|error| classify_error(&error)),
        )
    });
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
    (!cancel.load(Ordering::SeqCst)).then_some(Composed {
        segments: probed.segments,
        timeline,
        compositions,
        assets,
    })
}
