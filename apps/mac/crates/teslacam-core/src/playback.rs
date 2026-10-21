//! The playback intent machine of one theater session.
//!
//! The Mac plays the six cameras as one synchronized group (AVFoundation players on a shared host
//! clock), so the machine tracks the user's intent and the group's state instead of per-element play
//! attempts. It owns no media: it consumes user intents and media-layer reports and emits
//! [`MediaCommand`]s. Positions are seconds on the session's [`EventTimeline`].
//!
//! Rules (spec §1.18–§1.19 adapted to a synchronized group):
//!
//! - A new session intends to play (autoplay): the group starts as soon as one camera is ready, from
//!   the initial position (the Sentry pre-roll, else 0). It autoplays once; a repeated `ready` never
//!   restarts or rewinds it.
//! - `pause` before any camera is ready cancels autoplay.
//! - `play` with no ready camera shows [`PlaybackMessage::NoPlayable`]; playback then starts by itself
//!   when a camera becomes ready, or stops when every camera is terminal. Unlike the web, the message
//!   is never replaced by [`PlaybackMessage::Rejected`], which only reports a failed start.
//! - Seeking, skipping (clamped to the event) and rate changes keep the intent.
//! - At the end of the event the group pauses at the duration; `play` there restarts the last
//!   segment from its start.
//! - The representative view (`event.mp4`) pauses the six-camera clock. It is entered by the user, or
//!   automatically when every camera is terminal, at least one with a file fault, and `event.mp4`
//!   exists. Gaps where every camera is merely missing never switch views.

use crate::camera::{CameraId, CameraMap};
use crate::copy;
use crate::event_timeline::EventTimeline;

/// Playback rate choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PlaybackRate {
    /// 0.5×
    Half,
    /// 1× (the default of every session)
    #[default]
    Normal,
    /// 1.5×
    OneAndHalf,
    /// 2×
    Double,
}

impl PlaybackRate {
    /// Menu order.
    pub const ALL: [PlaybackRate; 4] = [
        PlaybackRate::Half,
        PlaybackRate::Normal,
        PlaybackRate::OneAndHalf,
        PlaybackRate::Double,
    ];

    /// The multiplier.
    pub const fn value(self) -> f64 {
        match self {
            PlaybackRate::Half => 0.5,
            PlaybackRate::Normal => 1.0,
            PlaybackRate::OneAndHalf => 1.5,
            PlaybackRate::Double => 2.0,
        }
    }

    /// The menu label, e.g. `1.5×`.
    pub fn label(self) -> String {
        copy::rate_label(self.value())
    }
}

/// The camera arrangement the user chose; kept across events, reset with a new folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StageLayout {
    /// Cameras around the car (`車両配置`).
    #[default]
    Vehicle,
    /// 3 × 2 grid (`6分割`).
    Grid,
}

/// What the stage shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewMode {
    /// Cameras around the car.
    Vehicle,
    /// 3 × 2 grid.
    Grid,
    /// The single-stream `event.mp4` (`代表映像`).
    Representative,
}

impl From<StageLayout> for ViewMode {
    fn from(layout: StageLayout) -> ViewMode {
        match layout {
            StageLayout::Vehicle => ViewMode::Vehicle,
            StageLayout::Grid => ViewMode::Grid,
        }
    }
}

/// What a camera window shows at the current position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TileStatus {
    /// The file is being opened.
    Loading,
    /// A frame can be shown.
    Ready,
    /// No footage at this time.
    Missing,
    /// The file cannot be read.
    Unavailable,
    /// Decode error.
    Corrupt,
    /// Unsupported format.
    Unsupported,
}

impl TileStatus {
    /// Neither loading nor ready.
    pub const fn is_terminal(self) -> bool {
        !matches!(self, TileStatus::Loading | TileStatus::Ready)
    }

    /// A file exists but cannot be shown.
    pub const fn is_fault(self) -> bool {
        matches!(
            self,
            TileStatus::Unavailable | TileStatus::Corrupt | TileStatus::Unsupported
        )
    }

    /// The overlay text; `None` when the window shows video.
    pub const fn label(self) -> Option<&'static str> {
        match self {
            TileStatus::Loading => Some(copy::TILE_LOADING),
            TileStatus::Ready => None,
            TileStatus::Missing => Some(copy::TILE_MISSING),
            TileStatus::Unavailable => Some(copy::TILE_UNAVAILABLE),
            TileStatus::Corrupt => Some(copy::TILE_CORRUPT),
            TileStatus::Unsupported => Some(copy::TILE_UNSUPPORTED),
        }
    }
}

/// The playback message under the transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaybackMessage {
    /// Play was requested and no camera can play.
    NoPlayable,
    /// The media layer could not start playback.
    Rejected,
}

impl PlaybackMessage {
    /// The text (may contain a line break).
    pub const fn text(self) -> &'static str {
        match self {
            PlaybackMessage::NoPlayable => copy::PLAYBACK_NO_PLAYABLE,
            PlaybackMessage::Rejected => copy::PLAYBACK_REJECTED,
        }
    }
}

/// Whether the group plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlaybackPhase {
    /// Not intended to play.
    Paused,
    /// Intended to play; waiting for a ready camera or for the start to be confirmed.
    Waiting,
    /// The group is running.
    Playing,
}

/// How a session starts.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionConfig {
    /// Event duration on the timeline.
    pub duration: f64,
    /// Segment starts on the timeline, ascending.
    pub segment_starts: Vec<f64>,
    /// Where playback starts.
    pub initial_position: f64,
    /// The workspace's stage layout.
    pub layout: StageLayout,
    /// Whether the event has `event.mp4`.
    pub has_representative: bool,
    /// Camera statuses at the initial position.
    pub statuses: CameraMap<TileStatus>,
}

impl SessionConfig {
    /// A config from the event's timeline: cameras with a clip at `initial_position` start
    /// [`TileStatus::Loading`], the others [`TileStatus::Missing`].
    pub fn new(
        timeline: &EventTimeline,
        initial_position: f64,
        layout: StageLayout,
        has_representative: bool,
    ) -> SessionConfig {
        let initial_position = timeline.clamp(initial_position);
        SessionConfig {
            duration: timeline.duration(),
            segment_starts: timeline
                .segments()
                .iter()
                .map(|segment| segment.start)
                .collect(),
            initial_position,
            layout,
            has_representative,
            statuses: CameraMap::from_fn(|camera| {
                if timeline.media_time(camera, initial_position).is_some() {
                    TileStatus::Loading
                } else {
                    TileStatus::Missing
                }
            }),
        }
    }
}

/// Inputs from the UI and the media layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackInput {
    /// `再生`.
    Play,
    /// `一時停止`.
    Pause,
    /// `Space`.
    TogglePlay,
    /// Scrubbing to a position.
    Seek(f64),
    /// `10秒戻る` / `10秒進む` (±10).
    Skip(f64),
    /// The rate menu.
    SetRate(PlaybackRate),
    /// A camera window or digit key: make it active and toggle its expansion.
    ToggleCamera(CameraId),
    /// `Esc`.
    Collapse,
    /// The view segmented control.
    SetViewMode(ViewMode),
    /// The media layer: a camera's status at the current position changed.
    CameraStatus(CameraId, TileStatus),
    /// The media layer: the group clock moved.
    Clock(f64),
    /// The media layer: the group reached the end of the event.
    ReachedEnd,
    /// The media layer: the requested start succeeded.
    Started,
    /// The media layer: the requested start failed.
    StartFailed,
}

/// Commands for the media layer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MediaCommand {
    /// Start the group at `from` with `rate`.
    Play {
        /// Timeline seconds.
        from: f64,
        /// Rate multiplier.
        rate: f64,
    },
    /// Pause the group.
    Pause,
    /// Move the group (keeps playing if it plays).
    Seek(f64),
    /// Change the rate.
    SetRate(f64),
}

/// What the session asks of its owner.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackEffect {
    /// Forward to the media layer.
    Media(MediaCommand),
    /// Remember the stage layout in the workspace.
    LayoutChosen(StageLayout),
}

/// The playback state of one theater session.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaybackMachine {
    duration: f64,
    segment_starts: Vec<f64>,
    position: f64,
    rate: PlaybackRate,
    intended: bool,
    start_requested: bool,
    phase: PlaybackPhase,
    message: Option<PlaybackMessage>,
    statuses: CameraMap<TileStatus>,
    layout: StageLayout,
    view_mode: ViewMode,
    has_representative: bool,
    expanded: Option<CameraId>,
    active: CameraId,
}

impl PlaybackMachine {
    /// A new session that intends to play, and the effects of its initial camera statuses.
    pub fn new(config: SessionConfig) -> (PlaybackMachine, Vec<PlaybackEffect>) {
        let mut machine = PlaybackMachine {
            duration: config.duration,
            segment_starts: config.segment_starts,
            position: 0.0,
            rate: PlaybackRate::Normal,
            intended: true,
            start_requested: false,
            phase: PlaybackPhase::Waiting,
            message: None,
            statuses: config.statuses,
            layout: config.layout,
            view_mode: config.layout.into(),
            has_representative: config.has_representative,
            expanded: None,
            active: CameraId::Front,
        };
        machine.position = machine.clamp(config.initial_position);
        let effects = machine.evaluate();
        (machine, effects)
    }

    /// Applies `input` and returns the effects.
    pub fn handle(&mut self, input: PlaybackInput) -> Vec<PlaybackEffect> {
        match input {
            PlaybackInput::Play => self.play(),
            PlaybackInput::Pause => self.stop(),
            PlaybackInput::TogglePlay if self.intended => self.stop(),
            PlaybackInput::TogglePlay => self.play(),
            PlaybackInput::Seek(seconds) => self.seek(seconds),
            PlaybackInput::Skip(seconds) => self.seek(self.position + seconds),
            PlaybackInput::SetRate(rate) => {
                self.rate = rate;
                vec![PlaybackEffect::Media(MediaCommand::SetRate(rate.value()))]
            }
            PlaybackInput::ToggleCamera(camera) => {
                if self.view_mode == ViewMode::Representative {
                    self.view_mode = self.layout.into();
                }
                self.active = camera;
                self.expanded = if self.expanded == Some(camera) {
                    None
                } else {
                    Some(camera)
                };
                Vec::new()
            }
            PlaybackInput::Collapse => {
                self.expanded = None;
                Vec::new()
            }
            PlaybackInput::SetViewMode(ViewMode::Representative) => {
                if self.has_representative && self.view_mode != ViewMode::Representative {
                    self.enter_representative()
                } else {
                    Vec::new()
                }
            }
            PlaybackInput::SetViewMode(mode) => {
                self.layout = if mode == ViewMode::Grid {
                    StageLayout::Grid
                } else {
                    StageLayout::Vehicle
                };
                self.view_mode = mode;
                self.expanded = None;
                vec![PlaybackEffect::LayoutChosen(self.layout)]
            }
            PlaybackInput::CameraStatus(camera, status) => {
                self.statuses[camera] = status;
                self.evaluate()
            }
            PlaybackInput::Clock(seconds) => {
                self.position = self.clamp(seconds);
                Vec::new()
            }
            PlaybackInput::ReachedEnd => {
                self.position = self.duration;
                self.stop()
            }
            PlaybackInput::Started if self.intended => {
                self.phase = PlaybackPhase::Playing;
                self.start_requested = false;
                self.message = None;
                Vec::new()
            }
            PlaybackInput::Started => vec![PlaybackEffect::Media(MediaCommand::Pause)],
            PlaybackInput::StartFailed => {
                if self.intended {
                    self.intended = false;
                    self.start_requested = false;
                    self.phase = PlaybackPhase::Paused;
                    self.message = Some(PlaybackMessage::Rejected);
                }
                Vec::new()
            }
        }
    }

    fn clamp(&self, seconds: f64) -> f64 {
        if seconds.is_nan() {
            0.0
        } else {
            seconds.clamp(0.0, self.duration)
        }
    }

    fn any(&self, status: TileStatus) -> bool {
        self.statuses.iter().any(|(_, current)| *current == status)
    }

    fn seek(&mut self, seconds: f64) -> Vec<PlaybackEffect> {
        self.position = self.clamp(seconds);
        vec![PlaybackEffect::Media(MediaCommand::Seek(self.position))]
    }

    fn start_command(&mut self) -> PlaybackEffect {
        self.start_requested = true;
        self.phase = PlaybackPhase::Waiting;
        PlaybackEffect::Media(MediaCommand::Play {
            from: self.position,
            rate: self.rate.value(),
        })
    }

    fn play(&mut self) -> Vec<PlaybackEffect> {
        if self.intended && (self.start_requested || self.phase == PlaybackPhase::Playing) {
            return Vec::new();
        }
        if self.view_mode == ViewMode::Representative {
            self.view_mode = self.layout.into();
        }
        self.intended = true;
        self.message = None;
        let mut effects = Vec::new();
        if self.duration > 0.0 && self.position >= self.duration {
            let last_start = self
                .segment_starts
                .iter()
                .rev()
                .copied()
                .find(|start| *start <= self.position)
                .unwrap_or(0.0);
            effects.extend(self.seek(last_start));
        }
        if self.any(TileStatus::Ready) {
            effects.push(self.start_command());
        } else {
            self.message = Some(PlaybackMessage::NoPlayable);
            if self.any(TileStatus::Loading) {
                self.phase = PlaybackPhase::Waiting;
            } else {
                self.intended = false;
                self.phase = PlaybackPhase::Paused;
            }
        }
        effects
    }

    fn stop(&mut self) -> Vec<PlaybackEffect> {
        let was_running =
            self.intended || self.start_requested || self.phase == PlaybackPhase::Playing;
        self.intended = false;
        self.start_requested = false;
        self.phase = PlaybackPhase::Paused;
        if was_running {
            vec![PlaybackEffect::Media(MediaCommand::Pause)]
        } else {
            Vec::new()
        }
    }

    fn enter_representative(&mut self) -> Vec<PlaybackEffect> {
        self.view_mode = ViewMode::Representative;
        self.expanded = None;
        self.stop()
    }

    /// Reacts to camera statuses: the automatic representative view, autoplay, or giving up.
    fn evaluate(&mut self) -> Vec<PlaybackEffect> {
        let all_terminal = self.statuses.iter().all(|(_, status)| status.is_terminal());
        let any_fault = self.statuses.iter().any(|(_, status)| status.is_fault());
        if self.has_representative
            && self.view_mode != ViewMode::Representative
            && all_terminal
            && any_fault
        {
            return self.enter_representative();
        }
        if !self.intended || self.start_requested || self.phase == PlaybackPhase::Playing {
            return Vec::new();
        }
        if self.any(TileStatus::Ready) {
            return vec![self.start_command()];
        }
        if all_terminal {
            self.intended = false;
            self.phase = PlaybackPhase::Paused;
            self.message = Some(PlaybackMessage::NoPlayable);
        }
        Vec::new()
    }

    /// Timeline seconds.
    pub fn position(&self) -> f64 {
        self.position
    }

    /// Event duration.
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// Group state.
    pub fn phase(&self) -> PlaybackPhase {
        self.phase
    }

    /// Whether the user wants playback (the transport shows `一時停止` then).
    pub fn intends_to_play(&self) -> bool {
        self.intended
    }

    /// Current rate.
    pub fn rate(&self) -> PlaybackRate {
        self.rate
    }

    /// The message under the transport.
    pub fn message(&self) -> Option<PlaybackMessage> {
        self.message
    }

    /// What the stage shows.
    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    /// The expanded camera.
    pub fn expanded_camera(&self) -> Option<CameraId> {
        self.expanded
    }

    /// The active camera (blue ring and wedge); `front` when a session starts.
    pub fn active_camera(&self) -> CameraId {
        self.active
    }

    /// A camera's status.
    pub fn status(&self, camera: CameraId) -> TileStatus {
        self.statuses[camera]
    }

    /// The camera whose clock the UI follows: the expanded camera when ready, else the first ready
    /// camera in canonical order.
    pub fn clock_camera(&self) -> Option<CameraId> {
        self.expanded
            .filter(|camera| self.statuses[*camera] == TileStatus::Ready)
            .or_else(|| {
                CameraId::ALL
                    .into_iter()
                    .find(|camera| self.statuses[*camera] == TileStatus::Ready)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use MediaCommand::{Pause, Play, Seek, SetRate};
    use PlaybackEffect::{LayoutChosen, Media};

    fn config(duration: f64, initial: f64) -> SessionConfig {
        SessionConfig {
            duration,
            segment_starts: (0..(duration / 60.0).ceil() as usize)
                .map(|index| index as f64 * 60.0)
                .collect(),
            initial_position: initial,
            layout: StageLayout::Vehicle,
            has_representative: false,
            statuses: CameraMap::from_fn(|_| TileStatus::Loading),
        }
    }

    fn session(duration: f64, initial: f64) -> PlaybackMachine {
        let (machine, effects) = PlaybackMachine::new(config(duration, initial));
        assert!(effects.is_empty());
        machine
    }

    fn playing(duration: f64) -> PlaybackMachine {
        let mut machine = session(duration, 0.0);
        machine.handle(PlaybackInput::CameraStatus(
            CameraId::Front,
            TileStatus::Ready,
        ));
        machine.handle(PlaybackInput::Started);
        machine
    }

    fn all(machine: &mut PlaybackMachine, status: TileStatus) -> Vec<PlaybackEffect> {
        CameraId::ALL
            .into_iter()
            .flat_map(|camera| machine.handle(PlaybackInput::CameraStatus(camera, status)))
            .collect()
    }

    #[test]
    fn a_new_session_autoplays_once_from_the_initial_position() {
        let mut machine = session(120.0, 65.0);
        assert_eq!(machine.phase(), PlaybackPhase::Waiting);
        assert!(machine.intends_to_play());
        assert_eq!(machine.position(), 65.0);
        assert_eq!(machine.rate(), PlaybackRate::Normal);
        assert_eq!(machine.active_camera(), CameraId::Front);
        assert_eq!(
            machine.handle(PlaybackInput::CameraStatus(
                CameraId::Back,
                TileStatus::Ready
            )),
            vec![Media(Play {
                from: 65.0,
                rate: 1.0
            })]
        );
        assert!(
            machine
                .handle(PlaybackInput::CameraStatus(
                    CameraId::Front,
                    TileStatus::Ready
                ))
                .is_empty()
        );
        assert!(machine.handle(PlaybackInput::Started).is_empty());
        assert_eq!(machine.phase(), PlaybackPhase::Playing);
        assert!(
            machine
                .handle(PlaybackInput::CameraStatus(
                    CameraId::Back,
                    TileStatus::Ready
                ))
                .is_empty()
        );
        assert_eq!(machine.position(), 65.0);
    }

    #[test]
    fn pausing_before_ready_cancels_autoplay() {
        let mut machine = session(60.0, 0.0);
        machine.handle(PlaybackInput::Pause);
        assert_eq!(machine.phase(), PlaybackPhase::Paused);
        assert!(all(&mut machine, TileStatus::Ready).is_empty());
        assert!(!machine.intends_to_play());
    }

    #[test]
    fn play_while_loading_waits_with_the_no_playable_message() {
        let mut machine = session(60.0, 0.0);
        machine.handle(PlaybackInput::Pause);
        assert!(machine.handle(PlaybackInput::Play).is_empty());
        assert_eq!(machine.message(), Some(PlaybackMessage::NoPlayable));
        assert_eq!(machine.phase(), PlaybackPhase::Waiting);
        assert_eq!(
            machine.handle(PlaybackInput::CameraStatus(
                CameraId::LeftPillar,
                TileStatus::Ready
            )),
            vec![Media(Play {
                from: 0.0,
                rate: 1.0
            })]
        );
        assert_eq!(machine.message(), Some(PlaybackMessage::NoPlayable));
        machine.handle(PlaybackInput::Started);
        assert_eq!(machine.message(), None);
        assert_eq!(
            PlaybackMessage::NoPlayable
                .text()
                .lines()
                .collect::<Vec<_>>(),
            [
                "再生できるカメラ映像がありません。",
                "読み込み後に再試行してください。"
            ]
        );
    }

    #[test]
    fn no_playable_is_not_overwritten_when_every_camera_is_terminal() {
        let mut machine = session(60.0, 0.0);
        machine.handle(PlaybackInput::Pause);
        all(&mut machine, TileStatus::Corrupt);
        assert!(machine.handle(PlaybackInput::Play).is_empty());
        assert_eq!(machine.message(), Some(PlaybackMessage::NoPlayable));
        assert_eq!(machine.phase(), PlaybackPhase::Paused);
        assert!(!machine.intends_to_play());
    }

    #[test]
    fn autoplay_stops_with_no_playable_when_every_camera_turns_terminal() {
        let mut machine = session(60.0, 0.0);
        all(&mut machine, TileStatus::Missing);
        assert_eq!(machine.phase(), PlaybackPhase::Paused);
        assert_eq!(machine.message(), Some(PlaybackMessage::NoPlayable));
    }

    #[test]
    fn a_failed_start_is_rejected_and_not_retried() {
        let mut machine = session(60.0, 0.0);
        machine.handle(PlaybackInput::CameraStatus(
            CameraId::Front,
            TileStatus::Ready,
        ));
        assert!(machine.handle(PlaybackInput::StartFailed).is_empty());
        assert_eq!(machine.message(), Some(PlaybackMessage::Rejected));
        assert_eq!(machine.phase(), PlaybackPhase::Paused);
        assert!(
            machine
                .handle(PlaybackInput::CameraStatus(
                    CameraId::Back,
                    TileStatus::Ready
                ))
                .is_empty()
        );
        assert_eq!(
            machine.handle(PlaybackInput::Play),
            vec![Media(Play {
                from: 0.0,
                rate: 1.0
            })]
        );
        assert_eq!(machine.message(), None);
    }

    #[test]
    fn seek_skip_rate_and_pause() {
        let mut machine = playing(120.0);
        assert_eq!(
            machine.handle(PlaybackInput::Seek(45.0)),
            vec![Media(Seek(45.0))]
        );
        assert_eq!(
            machine.handle(PlaybackInput::Skip(10.0)),
            vec![Media(Seek(55.0))]
        );
        assert_eq!(
            machine.handle(PlaybackInput::SetRate(PlaybackRate::OneAndHalf)),
            vec![Media(SetRate(1.5))]
        );
        assert_eq!(machine.phase(), PlaybackPhase::Playing);
        assert_eq!(machine.handle(PlaybackInput::Pause), vec![Media(Pause)]);
        assert_eq!(machine.position(), 55.0);
        assert_eq!(machine.rate(), PlaybackRate::OneAndHalf);
        assert_eq!(
            machine.handle(PlaybackInput::TogglePlay),
            vec![Media(Play {
                from: 55.0,
                rate: 1.5
            })]
        );
        machine.handle(PlaybackInput::Started);
        assert_eq!(
            machine.handle(PlaybackInput::TogglePlay),
            vec![Media(Pause)]
        );
    }

    #[test]
    fn a_pending_start_keeps_the_intent_across_seek_skip_and_rate() {
        let mut machine = session(120.0, 0.0);
        machine.handle(PlaybackInput::CameraStatus(
            CameraId::Front,
            TileStatus::Ready,
        ));
        assert_eq!(
            machine.handle(PlaybackInput::Seek(30.0)),
            vec![Media(Seek(30.0))]
        );
        assert_eq!(
            machine.handle(PlaybackInput::Skip(-10.0)),
            vec![Media(Seek(20.0))]
        );
        assert_eq!(
            machine.handle(PlaybackInput::SetRate(PlaybackRate::Double)),
            vec![Media(SetRate(2.0))]
        );
        machine.handle(PlaybackInput::Started);
        assert_eq!(machine.phase(), PlaybackPhase::Playing);
        assert_eq!(
            (machine.position(), machine.rate()),
            (20.0, PlaybackRate::Double)
        );
    }

    #[test]
    fn positions_are_clamped_to_the_event() {
        let mut machine = playing(60.0);
        assert_eq!(
            machine.handle(PlaybackInput::Skip(-10.0)),
            vec![Media(Seek(0.0))]
        );
        assert_eq!(
            machine.handle(PlaybackInput::Seek(1e9)),
            vec![Media(Seek(60.0))]
        );
        assert_eq!(
            machine.handle(PlaybackInput::Seek(f64::NAN)),
            vec![Media(Seek(0.0))]
        );
        machine.handle(PlaybackInput::Clock(75.0));
        assert_eq!(machine.position(), 60.0);
        machine.handle(PlaybackInput::Clock(12.5));
        assert_eq!(machine.position(), 12.5);
    }

    #[test]
    fn the_end_pauses_and_play_restarts_the_last_segment() {
        let mut machine = playing(180.0);
        assert_eq!(
            machine.handle(PlaybackInput::ReachedEnd),
            vec![Media(Pause)]
        );
        assert_eq!(machine.position(), 180.0);
        assert_eq!(machine.phase(), PlaybackPhase::Paused);
        assert_eq!(
            machine.handle(PlaybackInput::Play),
            vec![
                Media(Seek(120.0)),
                Media(Play {
                    from: 120.0,
                    rate: 1.0
                })
            ]
        );
    }

    #[test]
    fn a_start_confirmed_after_pause_is_undone() {
        let mut machine = session(60.0, 0.0);
        machine.handle(PlaybackInput::CameraStatus(
            CameraId::Front,
            TileStatus::Ready,
        ));
        assert_eq!(machine.handle(PlaybackInput::Pause), vec![Media(Pause)]);
        assert_eq!(machine.handle(PlaybackInput::Started), vec![Media(Pause)]);
        assert_eq!(machine.phase(), PlaybackPhase::Paused);
    }

    #[test]
    fn cameras_expand_and_collapse() {
        let mut machine = playing(60.0);
        assert!(
            machine
                .handle(PlaybackInput::ToggleCamera(CameraId::LeftPillar))
                .is_empty()
        );
        assert_eq!(
            (machine.active_camera(), machine.expanded_camera()),
            (CameraId::LeftPillar, Some(CameraId::LeftPillar))
        );
        machine.handle(PlaybackInput::ToggleCamera(CameraId::LeftPillar));
        assert_eq!(machine.expanded_camera(), None);
        machine.handle(PlaybackInput::ToggleCamera(CameraId::RightRepeater));
        machine.handle(PlaybackInput::ToggleCamera(CameraId::Back));
        assert_eq!(machine.expanded_camera(), Some(CameraId::Back));
        machine.handle(PlaybackInput::Collapse);
        assert_eq!(
            (machine.active_camera(), machine.expanded_camera()),
            (CameraId::Back, None)
        );
        assert_eq!(machine.phase(), PlaybackPhase::Playing);
    }

    #[test]
    fn view_modes() {
        let mut machine = playing(60.0);
        machine.handle(PlaybackInput::ToggleCamera(CameraId::Front));
        assert_eq!(
            machine.handle(PlaybackInput::SetViewMode(ViewMode::Grid)),
            vec![LayoutChosen(StageLayout::Grid)]
        );
        assert_eq!(
            (machine.view_mode(), machine.expanded_camera()),
            (ViewMode::Grid, None)
        );
        assert!(
            machine
                .handle(PlaybackInput::SetViewMode(ViewMode::Representative))
                .is_empty()
        );
        assert_eq!(machine.view_mode(), ViewMode::Grid);
        assert_eq!(machine.phase(), PlaybackPhase::Playing);
    }

    #[test]
    fn the_representative_view_pauses_the_clock() {
        let (mut machine, _) = PlaybackMachine::new(SessionConfig {
            has_representative: true,
            ..config(60.0, 0.0)
        });
        machine.handle(PlaybackInput::CameraStatus(
            CameraId::Front,
            TileStatus::Ready,
        ));
        machine.handle(PlaybackInput::Started);
        assert_eq!(
            machine.handle(PlaybackInput::SetViewMode(ViewMode::Representative)),
            vec![Media(Pause)]
        );
        assert_eq!(
            (machine.view_mode(), machine.phase()),
            (ViewMode::Representative, PlaybackPhase::Paused)
        );
        assert_eq!(
            machine.handle(PlaybackInput::Play),
            vec![Media(Play {
                from: 0.0,
                rate: 1.0
            })]
        );
        assert_eq!(machine.view_mode(), ViewMode::Vehicle);
        machine.handle(PlaybackInput::SetViewMode(ViewMode::Representative));
        machine.handle(PlaybackInput::ToggleCamera(CameraId::Back));
        assert_eq!(
            (machine.view_mode(), machine.expanded_camera()),
            (ViewMode::Vehicle, Some(CameraId::Back))
        );
    }

    #[test]
    fn faulty_files_switch_to_the_representative_view() {
        let (mut machine, _) = PlaybackMachine::new(SessionConfig {
            has_representative: true,
            ..config(60.0, 0.0)
        });
        let effects = all(&mut machine, TileStatus::Corrupt);
        assert_eq!(machine.view_mode(), ViewMode::Representative);
        assert!(!machine.intends_to_play());
        assert!(effects.iter().all(|effect| *effect == Media(Pause)));

        let (mut gap, _) = PlaybackMachine::new(SessionConfig {
            has_representative: true,
            ..config(60.0, 0.0)
        });
        all(&mut gap, TileStatus::Missing);
        assert_eq!(gap.view_mode(), ViewMode::Vehicle);

        let (mut unavailable, _) = PlaybackMachine::new(SessionConfig {
            has_representative: true,
            ..config(60.0, 0.0)
        });
        all(&mut unavailable, TileStatus::Missing);
        unavailable.handle(PlaybackInput::CameraStatus(
            CameraId::Front,
            TileStatus::Unavailable,
        ));
        assert_eq!(unavailable.view_mode(), ViewMode::Representative);
    }

    #[test]
    fn the_clock_follows_the_expanded_camera_when_ready() {
        let mut machine = session(60.0, 0.0);
        assert_eq!(machine.clock_camera(), None);
        machine.handle(PlaybackInput::CameraStatus(
            CameraId::Back,
            TileStatus::Ready,
        ));
        machine.handle(PlaybackInput::CameraStatus(
            CameraId::RightPillar,
            TileStatus::Ready,
        ));
        assert_eq!(machine.clock_camera(), Some(CameraId::Back));
        machine.handle(PlaybackInput::ToggleCamera(CameraId::RightPillar));
        assert_eq!(machine.clock_camera(), Some(CameraId::RightPillar));
        machine.handle(PlaybackInput::ToggleCamera(CameraId::Front));
        assert_eq!(machine.clock_camera(), Some(CameraId::Back));
    }

    #[test]
    fn a_session_config_follows_the_timeline() {
        use crate::event_timeline::SegmentMedia;
        use crate::timestamp::parse_filename_local_timestamp;

        let segment = |timestamp: &str, cameras: &[CameraId]| SegmentMedia {
            timestamp: parse_filename_local_timestamp(timestamp).expect("timestamp"),
            durations: CameraMap::from_fn(|camera| cameras.contains(&camera).then_some(60.0)),
        };
        let timeline = EventTimeline::new(&[
            segment("2026-08-21_12-00-00", &[CameraId::Front]),
            segment("2026-08-21_12-01-00", &[CameraId::Back]),
        ])
        .expect("timeline");
        let config = SessionConfig::new(&timeline, 65.0, StageLayout::Grid, true);
        assert_eq!(config.segment_starts, vec![0.0, 60.0]);
        assert_eq!(config.statuses[CameraId::Back], TileStatus::Loading);
        assert_eq!(config.statuses[CameraId::Front], TileStatus::Missing);
        let (machine, effects) = PlaybackMachine::new(config);
        assert!(effects.is_empty());
        assert_eq!(machine.view_mode(), ViewMode::Grid);
        assert_eq!(
            SessionConfig::new(&timeline, 500.0, StageLayout::Vehicle, false).initial_position,
            120.0
        );
    }

    #[test]
    fn rate_and_status_labels() {
        assert_eq!(
            PlaybackRate::ALL.map(PlaybackRate::label),
            ["0.5\u{00D7}", "1\u{00D7}", "1.5\u{00D7}", "2\u{00D7}"]
        );
        assert_eq!(TileStatus::Missing.label(), Some("記録がありません"));
        assert_eq!(TileStatus::Ready.label(), None);
        assert!(TileStatus::Missing.is_terminal() && !TileStatus::Missing.is_fault());
        assert!(TileStatus::Unsupported.is_fault());
    }
}
