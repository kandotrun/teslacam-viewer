//! The representative player for an event's `event.mp4` (single stream, not part of the sync).

use std::{path::Path, sync::mpsc, thread};

use objc2::{MainThreadMarker, rc::Retained};
use objc2_av_foundation::AVURLAsset;

use crate::{
    av::SendBox,
    engine::{Engine, Tile, TileState},
    frame::VideoFrame,
    playback::{PlaybackEvent, RATE_RANGE},
    probe::probe_file_blocking,
    status::CameraStatus,
    sync::{Command, Notice, SyncMachine},
    timeline::{ClipInfo, ClipProbe},
};

/// What the worker thread hands back: the probe and, when playable, the asset.
type Loaded = (ClipProbe, Option<Retained<AVURLAsset>>);

/// One `AVPlayer` for `event.mp4` with the same command and frame API as [`crate::EventPlayback`].
///
/// Threading and time work as for [`crate::EventPlayback`]: main thread only, non-blocking, driven by
/// [`RepresentativePlayer::tick`] once per frame, `now` in host seconds.
pub struct RepresentativePlayer {
    engine: Engine,
    loader: Option<mpsc::Receiver<SendBox<Loaded>>>,
    probe: Option<ClipProbe>,
    has_tile: bool,
    mtm: MainThreadMarker,
}

impl RepresentativePlayer {
    /// Starts loading `path`. Returns immediately.
    pub fn open(path: &Path, mtm: MainThreadMarker) -> RepresentativePlayer {
        let (tx, rx) = mpsc::channel();
        let path = path.to_path_buf();
        thread::spawn(move || {
            let probed = probe_file_blocking(&path);
            let _ = tx.send(SendBox((
                probed.probe,
                probed.media.map(|(asset, _)| asset),
            )));
        });
        RepresentativePlayer {
            engine: Engine::new(SyncMachine::new()),
            loader: Some(rx),
            probe: None,
            has_tile: false,
            mtm,
        }
    }

    /// Applies finished background work and advances seeks / starts. Call once per frame.
    pub fn tick(&mut self, now: f64) -> Vec<PlaybackEvent> {
        let mut events = Vec::new();
        if let Some(SendBox((probe, asset))) =
            self.loader.as_ref().and_then(|rx| rx.try_recv().ok())
        {
            self.loader = None;
            self.probe = Some(probe);
            let duration = probe.map_or(0.0, |info| info.duration);
            if let (Ok(_), Some(asset)) = (probe, asset) {
                let tile = Tile::new(&asset, self.engine.clock(), self.mtm);
                self.engine.attach(vec![tile], duration);
                self.has_tile = true;
            }
            events.push(PlaybackEvent::Loaded { duration });
        }
        if self.has_tile && self.engine.tick(now, |_, _| true) == Some(Notice::Ended) {
            events.push(PlaybackEvent::Ended);
        }
        events
    }

    /// Requests playback (from 0 when at the end).
    pub fn play(&mut self, now: f64) {
        self.engine.command(Command::Play, now);
    }

    /// Pauses.
    pub fn pause(&mut self, now: f64) {
        self.engine.command(Command::Pause, now);
    }

    /// Seeks exactly to `seconds` (clamped).
    pub fn seek(&mut self, seconds: f64, now: f64) {
        self.engine.command(Command::Seek(seconds), now);
    }

    /// Sets the playback rate, clamped to [`RATE_RANGE`].
    pub fn set_rate(&mut self, rate: f32, now: f64) {
        let rate = if rate.is_finite() {
            rate.clamp(RATE_RANGE.0, RATE_RANGE.1)
        } else {
            1.0
        };
        self.engine.command(Command::SetRate(rate), now);
    }

    /// True while playback is requested.
    pub fn is_playing(&self) -> bool {
        self.engine.machine().wants_play()
    }

    /// True once loaded and no seek or start is in flight.
    pub fn is_settled(&self) -> bool {
        self.engine.machine().is_settled()
    }

    /// Position at host time `now`.
    pub fn current_time(&self, now: f64) -> f64 {
        self.engine.machine().position(now)
    }

    /// Duration once loaded (0 when the file cannot be played).
    pub fn duration(&self) -> Option<f64> {
        self.probe
            .map(|probe| probe.map_or(0.0, |info| info.duration))
    }

    /// The probe result once loaded.
    pub fn info(&self) -> Option<ClipProbe> {
        self.probe
    }

    /// The window state: loading, ready or one of the faults.
    pub fn status(&mut self, _now: f64) -> CameraStatus {
        match self.probe {
            None => CameraStatus::Loading,
            Some(Err(fault)) => fault.into(),
            Some(Ok(_)) => match self.engine.tile_state(0) {
                TileState::Loading => CameraStatus::Loading,
                TileState::Ready => CameraStatus::Ready,
                TileState::Failed(fault) => fault.into(),
            },
        }
    }

    /// The frame shown at host time `now`.
    pub fn frame(&mut self, now: f64) -> Option<VideoFrame> {
        let ClipInfo {
            display_width,
            display_height,
            ..
        } = self.probe?.ok()?;
        let buffer = self.engine.pull(0, now)?.clone();
        Some(VideoFrame {
            buffer,
            display_width,
            display_height,
        })
    }
}
