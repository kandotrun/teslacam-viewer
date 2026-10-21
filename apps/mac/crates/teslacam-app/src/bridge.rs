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
}
