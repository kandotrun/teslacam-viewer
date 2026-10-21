//! The media of one theater session (effects only): the six synchronized players
//! (`teslacam_media::EventPlayback`), the `event.mp4` player of the representative view and the
//! composite export job. It holds no rules: [`crate::store::AppStore`] feeds its reports to the pure
//! playback and export machines and carries out their commands here.
//!
//! Main thread only, like the AVFoundation objects it owns. It lives in its own entity so that the
//! per-frame work (ticking players, pulling frames) only re-renders the stage and the transport.

use std::path::{Path, PathBuf};

use objc2::MainThreadMarker;
use teslacam_core::camera::{CameraId, CameraMap};
use teslacam_core::playback::{MediaCommand, TileStatus};
use teslacam_media::timeline::SegmentProbe;
use teslacam_media::{
    CompositeExporter, EventInput, EventPlayback, ExportError, ExportJob, ExportState,
    PlaybackEvent, RepresentativePlayer, VideoFrame,
};

use crate::bridge::{MediaCall, media_calls, tile_status};

/// What changed in the media since the previous [`SessionMedia::tick`].
#[derive(Debug, Clone)]
pub struct MediaReport {
    /// Probing finished: the per-file probe results and the event duration.
    pub loaded: Option<(Vec<SegmentProbe>, f64)>,
    /// Every camera's window state at the current position.
    pub statuses: CameraMap<TileStatus>,
    /// Playback reached the end of the event and paused there.
    pub ended: bool,
    /// The group runs on the shared clock.
    pub running: bool,
    /// News from the export job.
    pub export: Option<ExportUpdate>,
}

/// News from the export job.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportUpdate {
    /// Encoding progressed (0–1).
    Progressed(f64),
    /// The file is complete.
    Finished(PathBuf),
    /// The export stopped; `Aborted` after a cancel.
    Failed(ExportError),
}

/// The representative player and its file.
struct Representative {
    path: PathBuf,
    player: Option<RepresentativePlayer>,
}

/// The media of the selected event.
pub struct SessionMedia {
    event: EventInput,
    playback: EventPlayback,
    representative: Option<Representative>,
    export: Option<ExportJob>,
    mtm: MainThreadMarker,
}

impl SessionMedia {
    /// Starts loading `event` (positioned at `initial_position`, so autoplay starts there without a
    /// visible jump). `representative` is the event's `event.mp4`, opened when it is first shown.
    pub fn open(
        event: EventInput,
        initial_position: f64,
        representative: Option<PathBuf>,
        now: f64,
        mtm: MainThreadMarker,
    ) -> SessionMedia {
        let mut playback = EventPlayback::open(&event, mtm);
        if initial_position > 0.0 {
            playback.seek(initial_position, now);
        }
        SessionMedia {
            event,
            playback,
            representative: representative.map(|path| Representative { path, player: None }),
            export: None,
            mtm,
        }
    }

    /// Applies finished background work and reports what changed. Call once per frame while
    /// [`SessionMedia::needs_ticks`] holds.
    pub fn tick(&mut self, now: f64) -> MediaReport {
        let (mut loaded, mut ended) = (None, false);
        for event in self.playback.tick(now) {
            match event {
                PlaybackEvent::Loaded { duration } => {
                    let probe = self.playback.probe().map(<[_]>::to_vec).unwrap_or_default();
                    loaded = Some((probe, duration));
                }
                PlaybackEvent::Ended => ended = true,
            }
        }
        if let Some(player) = self.representative_player() {
            player.tick(now);
        }
        MediaReport {
            loaded,
            statuses: CameraMap::from_fn(|camera| tile_status(self.playback.status(camera, now))),
            ended,
            running: self.playback.is_running(),
            export: self.poll_export(),
        }
    }

    fn poll_export(&mut self) -> Option<ExportUpdate> {
        let job = self.export.as_mut()?;
        let update = match job.poll() {
            ExportState::Preparing => None,
            ExportState::Exporting { progress } => {
                Some(ExportUpdate::Progressed(f64::from(*progress)))
            }
            ExportState::Finished { path, .. } => Some(ExportUpdate::Finished(path.clone())),
            ExportState::Failed(error) => Some(ExportUpdate::Failed(error.clone())),
        };
        if job.state().is_done() {
            self.export = None;
        }
        update
    }

    /// Carries out a command of the playback machine.
    pub fn apply(&mut self, command: MediaCommand, now: f64) {
        let calls = media_calls(
            command,
            self.playback.current_time(now),
            self.playback.rate(),
        );
        for call in calls {
            match call {
                MediaCall::Seek(seconds) => self.playback.seek(seconds, now),
                MediaCall::SetRate(rate) => self.playback.set_rate(rate, now),
                MediaCall::Play => self.playback.play(now),
                MediaCall::Pause => self.playback.pause(now),
            }
        }
    }

    /// The group's position at host time `now`.
    pub fn position(&self, now: f64) -> f64 {
        self.playback.current_time(now)
    }

    /// Whether the frame loop must keep running: playing, loading, settling a seek, exporting, or
    /// the representative video playing or settling.
    pub fn needs_ticks(&self) -> bool {
        let representative = self
            .representative
            .as_ref()
            .and_then(|representative| representative.player.as_ref())
            .is_some_and(|player| player.is_playing() || !player.is_settled());
        self.playback.is_playing()
            || !self.playback.is_settled()
            || self.export.is_some()
            || representative
    }

    /// The frame `camera` shows at host time `now` (`None` in a gap, before loading, on a fault).
    pub fn frame(&mut self, camera: CameraId, now: f64) -> Option<VideoFrame> {
        self.playback.frame(camera, now)
    }

    /// Display size of `camera`'s first clip, once probed.
    pub fn display_size(&self, camera: CameraId) -> Option<(f64, f64)> {
        self.playback.timeline()?.cameras[camera].display_size()
    }

    // --- Representative video -----------------------------------------------------------------

    /// Opens `event.mp4` the first time the representative view is shown.
    pub fn ensure_representative(&mut self) {
        let mtm = self.mtm;
        if let Some(representative) = &mut self.representative
            && representative.player.is_none()
        {
            representative.player = Some(RepresentativePlayer::open(&representative.path, mtm));
        }
    }

    fn representative_player(&mut self) -> Option<&mut RepresentativePlayer> {
        self.representative.as_mut()?.player.as_mut()
    }

    /// The representative video's window state, frame, position and duration at `now`.
    pub fn representative_view(&mut self, now: f64) -> Option<RepresentativeView> {
        let player = self.representative_player()?;
        Some(RepresentativeView {
            status: tile_status(player.status(now)),
            frame: player.frame(now),
            position: player.current_time(now),
            duration: player.duration(),
            playing: player.is_playing(),
        })
    }

    /// Play / pause of the representative video's own controls.
    pub fn toggle_representative(&mut self, now: f64) {
        if let Some(player) = self.representative_player() {
            if player.is_playing() {
                player.pause(now);
            } else {
                player.play(now);
            }
        }
    }

    /// Pauses the representative video (when the stage leaves the representative view).
    pub fn pause_representative(&mut self, now: f64) {
        if let Some(player) = self.representative_player()
            && player.is_playing()
        {
            player.pause(now);
        }
    }

    /// Seeks the representative video.
    pub fn seek_representative(&mut self, seconds: f64, now: f64) {
        if let Some(player) = self.representative_player() {
            player.seek(seconds, now);
        }
    }

    // --- Export ---------------------------------------------------------------------------------

    /// Starts the composite export of the session's event to `output`.
    pub fn start_export(&mut self, output: &Path) {
        self.export = Some(CompositeExporter::new().start(&self.event, output, |_| {}, self.mtm));
    }

    /// Stops a running export (its partial file is deleted once AVFoundation has stopped).
    pub fn cancel_export(&mut self) {
        if let Some(job) = &mut self.export {
            job.cancel();
        }
    }
}

/// What the representative window draws.
pub struct RepresentativeView {
    /// Loading, ready or a fault.
    pub status: TileStatus,
    /// The current frame.
    pub frame: Option<VideoFrame>,
    /// Position in seconds.
    pub position: f64,
    /// Duration once loaded.
    pub duration: Option<f64>,
    /// Whether it plays.
    pub playing: bool,
}
