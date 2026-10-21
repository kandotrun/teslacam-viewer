//! Plain input data for this crate.
//!
//! Camera ids and per-camera maps are `teslacam-core`'s ([`CameraId`], [`CameraMap`]), so the canonical
//! order, the contract ids and the Japanese names are defined once. An [`EventInput`] is an event's
//! segments placed on the wall-clock timeline; [`EventInput::from_event`] builds it from a core
//! [`Event`] and its [`EventTimeline`].

use std::path::PathBuf;

pub use teslacam_core::camera::{CameraId, CameraMap};
use teslacam_core::event_timeline::EventTimeline;
use teslacam_core::library::{Event, Library};

/// One segment of an event: the clips that share one file-name timestamp.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SegmentInput {
    /// Wall-clock offset of this segment's timestamp from the event's first segment, in seconds
    /// (0 for the first segment).
    pub start_seconds: f64,
    /// The clip file per camera; `None` when the segment has no file for that camera.
    pub files: CameraMap<Option<PathBuf>>,
}

/// An event as the media layer sees it: its segments in ascending timestamp order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EventInput {
    /// Segments in ascending order of `start_seconds`.
    pub segments: Vec<SegmentInput>,
}

impl EventInput {
    /// The media input of `event` (a member of `library`): every segment starts where `timeline`
    /// places it, and each camera file is resolved to its path on disk.
    ///
    /// `timeline` must be the event's own timeline (for example
    /// [`EventTimeline::assuming_full_windows`]); segments beyond the shorter of the two are
    /// dropped.
    pub fn from_event(event: &Event, library: &Library, timeline: &EventTimeline) -> EventInput {
        EventInput {
            segments: event
                .segments
                .iter()
                .zip(timeline.segments())
                .map(|(segment, placed)| SegmentInput {
                    start_seconds: placed.start,
                    files: segment
                        .cameras
                        .map(|_, file| file.map(|id| library.file(id).path.clone())),
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use teslacam_core::camera::ClipCategory;
    use teslacam_core::library::{FileEntry, FileId, Segment};

    use super::*;

    fn library() -> Library {
        let files = ["a-front", "a-back", "b-front"]
            .iter()
            .map(|name| FileEntry {
                path: PathBuf::from(format!("/clips/{name}.mp4")),
                ..FileEntry::new(format!("{name}.mp4"), format!("TeslaCam/{name}.mp4"), 1)
            })
            .collect();
        let mut first = CameraMap::default();
        first[CameraId::Front] = Some(FileId(0));
        first[CameraId::Back] = Some(FileId(1));
        let mut second = CameraMap::default();
        second[CameraId::Front] = Some(FileId(2));
        Library {
            files,
            events: vec![Event {
                id: "saved-2026-08-21_12-00-00-1".to_owned(),
                category: ClipCategory::Saved,
                timestamp: "2026-08-21_12-00-00".to_owned(),
                segments: vec![
                    Segment {
                        timestamp: "2026-08-21_12-00-00".to_owned(),
                        cameras: first,
                    },
                    Segment {
                        timestamp: "2026-08-21_12-01-30".to_owned(),
                        cameras: second,
                    },
                ],
                representative: None,
                thumbnail: None,
                metadata: None,
            }],
            warnings: Vec::new(),
        }
    }

    #[test]
    fn events_take_their_segment_starts_from_the_core_timeline() {
        let library = library();
        let event = &library.events[0];
        let timeline = EventTimeline::assuming_full_windows(event).expect("timeline");
        let input = EventInput::from_event(event, &library, &timeline);
        assert_eq!(
            input
                .segments
                .iter()
                .map(|segment| segment.start_seconds)
                .collect::<Vec<_>>(),
            [0.0, 90.0]
        );
        assert_eq!(
            input.segments[0].files[CameraId::Back].as_deref(),
            Some(std::path::Path::new("/clips/a-back.mp4"))
        );
        assert_eq!(input.segments[1].files[CameraId::Back], None);
        assert_eq!(
            input.segments[1].files[CameraId::Front].as_deref(),
            Some(std::path::Path::new("/clips/b-front.mp4"))
        );
    }

    #[test]
    fn camera_ids_are_the_core_ids() {
        assert_eq!(CameraId::from_id("left_pillar"), Some(CameraId::LeftPillar));
        assert_eq!(CameraId::Back.label(), "リア");
        assert_eq!(
            CameraMap::from_fn(CameraId::index)[CameraId::RightRepeater],
            5
        );
    }
}
