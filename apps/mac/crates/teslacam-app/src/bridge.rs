//! The pure glue between the playback machine (`teslacam_core::playback`, state and rules) and the
//! media layer (`teslacam_media`, effects): which media calls a [`MediaCommand`] needs, and which
//! [`PlaybackInput`]s the media layer's reports turn into.

use teslacam_core::camera::{CameraId, CameraMap};
use teslacam_core::playback::{MediaCommand, PlaybackInput, TileStatus};
use teslacam_media::CameraStatus;

/// The window state of a media camera status.
pub fn tile_status(status: CameraStatus) -> TileStatus {
    match status {
        CameraStatus::Loading => TileStatus::Loading,
        CameraStatus::Ready => TileStatus::Ready,
        CameraStatus::Missing => TileStatus::Missing,
        CameraStatus::Unavailable => TileStatus::Unavailable,
        CameraStatus::Corrupt => TileStatus::Corrupt,
        CameraStatus::Unsupported => TileStatus::Unsupported,
    }
}

/// `CameraStatus` inputs for every camera whose status differs from what the machine was told,
/// in canonical order.
pub fn status_inputs(
    reported: &CameraMap<TileStatus>,
    current: &CameraMap<TileStatus>,
) -> Vec<PlaybackInput> {
    CameraId::ALL
        .into_iter()
        .filter(|camera| reported[*camera] != current[*camera])
        .map(|camera| PlaybackInput::CameraStatus(camera, current[camera]))
        .collect()
}

/// One call on `teslacam_media::EventPlayback`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MediaCall {
    /// `seek(seconds)`.
    Seek(f64),
    /// `set_rate(rate)`.
    SetRate(f32),
    /// `play()`.
    Play,
    /// `pause()`.
    Pause,
}

/// Positions closer than this are the same frame (a 1 ms tolerance; clips are 24–60 fps).
const SAME_POSITION: f64 = 1e-3;

/// The calls that carry out `command` on a group that is at `position` with `rate`.
pub fn media_calls(command: MediaCommand, position: f64, rate: f32) -> Vec<MediaCall> {
    match command {
        MediaCommand::Play { from, rate: wanted } => {
            let mut calls = Vec::new();
            if (position - from).abs() > SAME_POSITION {
                calls.push(MediaCall::Seek(from));
            }
            let wanted = wanted as f32;
            if (rate - wanted).abs() > f32::EPSILON {
                calls.push(MediaCall::SetRate(wanted));
            }
            calls.push(MediaCall::Play);
            calls
        }
        MediaCommand::Pause => vec![MediaCall::Pause],
        MediaCommand::Seek(seconds) => vec![MediaCall::Seek(seconds)],
        MediaCommand::SetRate(wanted) => vec![MediaCall::SetRate(wanted as f32)],
    }
}

/// What the media is doing, for deciding whether the frame loop must run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoopState {
    /// The clips are still being probed and composed.
    pub probing: bool,
    /// The group runs on the shared clock.
    pub running: bool,
    /// No seek or start is in flight.
    pub settled: bool,
    /// Some camera is loading or ready (not every one is missing or broken).
    pub live_cameras: bool,
    /// A composite export runs.
    pub exporting: bool,
    /// The representative video plays or settles a seek.
    pub representative_busy: bool,
}

/// Whether the frame loop must tick: something can still change on screen. A group that cannot
/// settle because no camera can show video (every file broken) does not keep the loop alive.
pub fn frame_loop_needed(state: LoopState) -> bool {
    state.probing
        || state.running
        || state.exporting
        || state.representative_busy
        || (!state.settled && state.live_cameras)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_map_one_to_one() {
        assert_eq!(tile_status(CameraStatus::Loading), TileStatus::Loading);
        assert_eq!(tile_status(CameraStatus::Ready), TileStatus::Ready);
        assert_eq!(tile_status(CameraStatus::Missing), TileStatus::Missing);
        assert_eq!(
            tile_status(CameraStatus::Unavailable),
            TileStatus::Unavailable
        );
        assert_eq!(tile_status(CameraStatus::Corrupt), TileStatus::Corrupt);
        assert_eq!(
            tile_status(CameraStatus::Unsupported),
            TileStatus::Unsupported
        );
    }

    #[test]
    fn only_changed_statuses_are_reported() {
        let reported = CameraMap::from_fn(|_| TileStatus::Loading);
        let mut current = reported;
        assert!(status_inputs(&reported, &current).is_empty());
        current[CameraId::RightRepeater] = TileStatus::Missing;
        current[CameraId::Back] = TileStatus::Ready;
        assert_eq!(
            status_inputs(&reported, &current),
            vec![
                PlaybackInput::CameraStatus(CameraId::Back, TileStatus::Ready),
                PlaybackInput::CameraStatus(CameraId::RightRepeater, TileStatus::Missing),
            ]
        );
    }

    #[test]
    fn play_moves_and_retimes_the_group_only_when_needed() {
        let play = |from, rate| MediaCommand::Play { from, rate };
        assert_eq!(
            media_calls(play(15.0, 1.0), 15.0, 1.0),
            vec![MediaCall::Play]
        );
        assert_eq!(
            media_calls(play(120.0, 1.5), 180.0, 1.0),
            vec![
                MediaCall::Seek(120.0),
                MediaCall::SetRate(1.5),
                MediaCall::Play
            ]
        );
        assert_eq!(
            media_calls(MediaCommand::Seek(42.0), 0.0, 1.0),
            vec![MediaCall::Seek(42.0)]
        );
        assert_eq!(
            media_calls(MediaCommand::SetRate(0.5), 0.0, 1.0),
            vec![MediaCall::SetRate(0.5)]
        );
        assert_eq!(
            media_calls(MediaCommand::Pause, 3.0, 1.0),
            vec![MediaCall::Pause]
        );
    }

    #[test]
    fn the_frame_loop_runs_only_while_something_can_change() {
        let idle = LoopState {
            probing: false,
            running: false,
            settled: true,
            live_cameras: true,
            exporting: false,
            representative_busy: false,
        };
        assert!(!frame_loop_needed(idle));
        // Probing, running (also through a gap where every camera is missing), exporting, or the
        // representative video playing keep it going.
        assert!(frame_loop_needed(LoopState {
            probing: true,
            ..idle
        }));
        assert!(frame_loop_needed(LoopState {
            running: true,
            live_cameras: false,
            ..idle
        }));
        assert!(frame_loop_needed(LoopState {
            exporting: true,
            ..idle
        }));
        assert!(frame_loop_needed(LoopState {
            representative_busy: true,
            ..idle
        }));
        // A seek or start settles while some camera can still show video ...
        assert!(frame_loop_needed(LoopState {
            settled: false,
            ..idle
        }));
        // ... but never settles when no camera can (every file is broken).
        assert!(!frame_loop_needed(LoopState {
            settled: false,
            live_cameras: false,
            ..idle
        }));
    }
}
