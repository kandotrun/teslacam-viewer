//! The theater session of the selected event: its wall-clock timeline, the playback machine and the
//! export machine (all pure, from `teslacam-core`). The media layer ([`crate::media`]) consumes the
//! [`MediaCommand`]s and reports camera statuses, the clock and the probed clip lengths back.

use teslacam_core::detection::initial_position;
use teslacam_core::event_timeline::{EventTimeline, SegmentMedia};
use teslacam_core::export_state::{ExportMachine, export_available};
use teslacam_core::library::Event;
use teslacam_core::playback::{
    MediaCommand, PlaybackEffect, PlaybackInput, PlaybackMachine, SessionConfig, StageLayout,
};
use teslacam_core::timestamp::parse_filename_local_timestamp;
use teslacam_media::timeline::SegmentProbe;

/// Everything owned by the selected event.
#[derive(Debug, Clone)]
pub struct Session {
    /// The event's timeline before clip lengths are known.
    pub timeline: EventTimeline,
    /// Play intent, position, rate, view mode, camera statuses.
    pub playback: PlaybackMachine,
    /// The composite export control.
    pub export: ExportMachine,
}

/// What a playback input asks of the app.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionEffects {
    /// Commands for the media layer.
    pub media: Vec<MediaCommand>,
    /// A layout the workspace should remember.
    pub layout: Option<StageLayout>,
}

impl SessionEffects {
    fn from_playback(effects: Vec<PlaybackEffect>) -> SessionEffects {
        let mut result = SessionEffects::default();
        for effect in effects {
            match effect {
                PlaybackEffect::Media(command) => result.media.push(command),
                PlaybackEffect::LayoutChosen(layout) => result.layout = Some(layout),
            }
        }
        result
    }
}

impl Session {
    /// A new session for `event` with the workspace's `layout`, starting at the Sentry pre-roll (or
    /// 0) and intending to play. `None` when the event's segment timestamps do not form a timeline.
    pub fn new(event: &Event, layout: StageLayout) -> Option<(Session, SessionEffects)> {
        let timeline = EventTimeline::assuming_full_windows(event).ok()?;
        let start = initial_position(event, &timeline);
        let config = SessionConfig::new(&timeline, start, layout, event.representative.is_some());
        let (playback, effects) = PlaybackMachine::new(config);
        Some((
            Session {
                timeline,
                playback,
                export: ExportMachine::new(export_available(event)),
            },
            SessionEffects::from_playback(effects),
        ))
    }

    /// Applies a playback input.
    pub fn handle(&mut self, input: PlaybackInput) -> SessionEffects {
        SessionEffects::from_playback(self.playback.handle(input))
    }

    /// The media layer probed `event`'s clips: the timeline takes the real clip lengths (faulty
    /// files leave gaps) and the playback machine the media layer's `duration`.
    pub fn media_loaded(
        &mut self,
        event: &Event,
        probe: &[SegmentProbe],
        duration: f64,
    ) -> SessionEffects {
        let segments: Option<Vec<SegmentMedia>> = event
            .segments
            .iter()
            .zip(probe)
            .map(|(segment, probed)| {
                Some(SegmentMedia {
                    timestamp: parse_filename_local_timestamp(&segment.timestamp)?,
                    durations: probed
                        .cameras
                        .map(|_, clip| clip.and_then(Result::ok).map(|info| info.duration)),
                })
            })
            .collect();
        if let Some(timeline) = segments
            .filter(|segments| segments.len() == event.segments.len())
            .and_then(|segments| EventTimeline::new(&segments).ok())
            .filter(EventTimeline::has_media)
        {
            self.timeline = timeline;
        }
        self.handle(PlaybackInput::Loaded { duration })
    }
}

#[cfg(test)]
mod tests {
    use teslacam_core::camera::{CameraId, CameraMap, ClipCategory};
    use teslacam_core::library::{FileId, Segment};
    use teslacam_core::metadata::EventMetadata;
    use teslacam_core::playback::{TileStatus, ViewMode};

    use super::*;

    fn event(category: ClipCategory, detection: Option<&str>, representative: bool) -> Event {
        let mut cameras = CameraMap::from_fn(|camera| Some(FileId(camera.index())));
        cameras[CameraId::RightRepeater] = None;
        Event {
            id: format!("{}-2026-08-20_20-35-00-1", category.id()),
            category,
            timestamp: "2026-08-20_20-35-00".into(),
            segments: vec![Segment {
                timestamp: "2026-08-20_20-35-00".into(),
                cameras,
            }],
            representative: representative.then_some(FileId(9)),
            thumbnail: None,
            metadata: detection.map(|timestamp| EventMetadata {
                timestamp: timestamp.into(),
                ..EventMetadata::default()
            }),
        }
    }

    #[test]
    fn a_sentry_session_starts_fifteen_seconds_before_the_detection() {
        let (session, _) = Session::new(
            &event(ClipCategory::Sentry, Some("2026-08-20T20:35:30"), false),
            StageLayout::Vehicle,
        )
        .expect("session");
        assert_eq!(session.playback.position(), 15.0);
        assert!(session.playback.intends_to_play());
        assert_eq!(session.playback.view_mode(), ViewMode::Vehicle);
        assert_eq!(
            session.playback.status(CameraId::RightRepeater),
            TileStatus::Missing
        );
        assert_eq!(
            session.playback.status(CameraId::Front),
            TileStatus::Loading
        );
        assert_eq!(session.export.unavailable_reason(), None);
    }

    #[test]
    fn other_categories_start_at_zero_in_the_workspace_layout() {
        let (session, _) = Session::new(
            &event(ClipCategory::Saved, Some("2026-08-20T20:35:30"), true),
            StageLayout::Grid,
        )
        .expect("session");
        assert_eq!(session.playback.position(), 0.0);
        assert_eq!(session.playback.view_mode(), ViewMode::Grid);
    }

    #[test]
    fn the_probed_clip_lengths_replace_the_assumed_windows() {
        use teslacam_media::FileFault;
        use teslacam_media::timeline::{ClipInfo, SegmentProbe};

        let mut event = event(ClipCategory::Recent, None, false);
        event.segments.push(Segment {
            timestamp: "2026-08-20_20-36-00".into(),
            cameras: CameraMap::from_fn(|camera| Some(FileId(10 + camera.index()))),
        });
        let (mut session, _) = Session::new(&event, StageLayout::Vehicle).expect("session");
        assert_eq!(session.timeline.duration(), 120.0);
        let clip = |duration| {
            Some(Ok(ClipInfo {
                source_start: 0.0,
                duration,
                display_width: 1280.0,
                display_height: 960.0,
                frame_duration: 1.0 / 36.0,
            }))
        };
        let probe = [
            SegmentProbe {
                start_seconds: 0.0,
                cameras: CameraMap::from_fn(|_| clip(60.0)),
            },
            SegmentProbe {
                start_seconds: 60.0,
                cameras: CameraMap::from_fn(|camera| match camera {
                    CameraId::Front => clip(42.0),
                    CameraId::Back => Some(Err(FileFault::Corrupt)),
                    _ => clip(40.0),
                }),
            },
        ];
        session.media_loaded(&event, &probe, 102.0);
        assert_eq!(session.timeline.duration(), 102.0);
        assert_eq!(session.playback.duration(), 102.0);
        assert!(session.timeline.media_time(CameraId::Back, 70.0).is_none());
        assert!(session.timeline.media_time(CameraId::Front, 70.0).is_some());
    }

    #[test]
    fn choosing_a_view_tells_the_workspace_its_layout() {
        let (mut session, _) = Session::new(
            &event(ClipCategory::Recent, None, true),
            StageLayout::Vehicle,
        )
        .expect("session");
        let effects = session.handle(PlaybackInput::SetViewMode(ViewMode::Grid));
        assert_eq!(effects.layout, Some(StageLayout::Grid));
        assert_eq!(session.playback.view_mode(), ViewMode::Grid);
        session.handle(PlaybackInput::SetViewMode(ViewMode::Representative));
        assert_eq!(session.playback.view_mode(), ViewMode::Representative);
    }
}
