//! The Mac's wall-clock event timeline.
//!
//! Unlike the web's nominal 60 s windows ([`crate::timeline`]), the Mac places every segment at its
//! real start time:
//!
//! - segment *i* starts at `ordinal(timestamp_i) − ordinal(timestamp_0)` seconds;
//! - each camera's file sits at its segment's start and is truncated where the next segment starts;
//! - a missing file, a file that cannot be played or the end of a short clip leaves a gap for that
//!   camera (gaps are part of the event time: the clock keeps running through them);
//! - a tail shorter than [`HOLD_TOLERANCE_SECONDS`] between the end of a clip and the end of its
//!   window holds the clip's last frame instead of becoming a gap, so a 59.97 s clip followed by a
//!   segment at 60 s never flashes a missing window (the same rule as `teslacam-media`'s timeline);
//! - the event lasts until the start of the last segment plus its longest clip, or
//!   [`NOMINAL_SEGMENT_SECONDS`] when no clip length of the last segment is known.
//!
//! For a regular 60 s cadence with 60 s clips this is exactly the web's 60 s math (tested against
//! `playback-timeline.json`).

use crate::camera::{CameraId, CameraMap};
use crate::library::Event;
use crate::timestamp::{LocalTimestamp, parse_filename_local_timestamp};

/// Length assumed for a segment whose clip lengths are unknown.
pub const NOMINAL_SEGMENT_SECONDS: f64 = 60.0;

/// Tails shorter than this hold the clip's last frame instead of becoming a gap.
pub const HOLD_TOLERANCE_SECONDS: f64 = 1.0;

/// One segment's start and the media length of each camera's clip.
#[derive(Debug, Clone, PartialEq)]
pub struct SegmentMedia {
    /// The segment timestamp.
    pub timestamp: LocalTimestamp,
    /// Playable media length per camera in seconds; `None` = no playable file.
    pub durations: CameraMap<Option<f64>>,
}

/// A segment on the event axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimelineSegment {
    /// The segment timestamp.
    pub timestamp: LocalTimestamp,
    /// Event seconds at which the segment starts.
    pub start: f64,
    /// Event seconds at which the next segment starts (the event end for the last one).
    pub end: f64,
}

/// A camera clip placed on the event axis. The clip plays from its media time 0.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    /// Index of the segment the clip belongs to.
    pub segment_index: usize,
    /// Event seconds at which the clip starts.
    pub start: f64,
    /// Seconds of the clip that are used (after truncation at the next segment).
    pub duration: f64,
    /// Seconds after the clip during which its last frame is held to close a tail shorter than
    /// [`HOLD_TOLERANCE_SECONDS`] (0 when there is none).
    pub hold: f64,
}

impl Placement {
    /// Event seconds at which the clip, including its hold, stops covering the camera.
    pub fn end(&self) -> f64 {
        self.start + self.duration + self.hold
    }
}

/// A position as segment index and offset into that segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimelinePosition {
    /// Index of the segment.
    pub segment_index: usize,
    /// Seconds after the segment's start.
    pub offset: f64,
}

/// Why a timeline cannot be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimelineError {
    /// Segment timestamps are not strictly ascending.
    UnorderedSegments,
    /// A segment timestamp is not a valid file-name timestamp.
    InvalidTimestamp,
}

/// The wall-clock timeline of one event.
#[derive(Debug, Clone, PartialEq)]
pub struct EventTimeline {
    segments: Vec<TimelineSegment>,
    placements: CameraMap<Vec<Placement>>,
    duration: f64,
}

fn usable_length(duration: Option<f64>) -> Option<f64> {
    duration.filter(|duration| duration.is_finite() && *duration > 0.0)
}

impl EventTimeline {
    /// Builds the timeline from segment starts and clip lengths.
    ///
    /// Lengths that are not finite and positive count as "no playable file".
    pub fn new(segments: &[SegmentMedia]) -> Result<EventTimeline, TimelineError> {
        let Some(first) = segments.first() else {
            return Ok(EventTimeline {
                segments: Vec::new(),
                placements: CameraMap::default(),
                duration: 0.0,
            });
        };
        let origin = first.timestamp.ordinal_seconds();
        let starts = segments
            .iter()
            .map(|segment| (segment.timestamp.ordinal_seconds() - origin) as f64)
            .collect::<Vec<_>>();
        if starts.windows(2).any(|pair| pair[1] <= pair[0]) {
            return Err(TimelineError::UnorderedSegments);
        }
        let last = segments.len() - 1;
        let has_media = segments.iter().any(|segment| {
            segment
                .durations
                .iter()
                .any(|(_, duration)| usable_length(*duration).is_some())
        });
        // The last window lasts as long as its longest clip; one without any playable clip adds no
        // length (unless nothing is known yet, then it is nominal).
        let last_length = segments[last]
            .durations
            .iter()
            .filter_map(|(_, duration)| usable_length(*duration))
            .reduce(f64::max)
            .unwrap_or(if has_media {
                0.0
            } else {
                NOMINAL_SEGMENT_SECONDS
            });
        let window_end = starts[last] + last_length;
        let mut timeline_segments = segments
            .iter()
            .zip(&starts)
            .enumerate()
            .map(|(index, (segment, start))| TimelineSegment {
                timestamp: segment.timestamp,
                start: *start,
                end: starts.get(index + 1).copied().unwrap_or(window_end),
            })
            .collect::<Vec<_>>();
        let placements = CameraMap::from_fn(|camera| {
            segments
                .iter()
                .zip(&timeline_segments)
                .enumerate()
                .filter_map(|(segment_index, (segment, window))| {
                    let length =
                        usable_length(segment.durations[camera])?.min(window.end - window.start);
                    let tail = window.end - window.start - length;
                    (length > 0.0).then_some(Placement {
                        segment_index,
                        start: window.start,
                        duration: length,
                        hold: if tail > 0.0 && tail < HOLD_TOLERANCE_SECONDS {
                            tail
                        } else {
                            0.0
                        },
                    })
                })
                .collect()
        });
        // The event ends where its latest video ends (as the media timeline and the export do).
        let duration = if has_media {
            placements
                .iter()
                .filter_map(|(_, placements): (CameraId, &Vec<Placement>)| placements.last())
                .map(|placement| placement.start + placement.duration + placement.hold)
                .fold(0.0, f64::max)
        } else {
            window_end
        };
        for segment in &mut timeline_segments {
            segment.end = segment.end.min(duration).max(segment.start);
        }
        Ok(EventTimeline {
            segments: timeline_segments,
            placements,
            duration,
        })
    }

    /// The timeline of `event` before clip lengths are known: every clip is assumed to fill its
    /// segment window (the last one [`NOMINAL_SEGMENT_SECONDS`]).
    pub fn assuming_full_windows(event: &Event) -> Result<EventTimeline, TimelineError> {
        let timestamps = event
            .segments
            .iter()
            .map(|segment| {
                parse_filename_local_timestamp(&segment.timestamp)
                    .ok_or(TimelineError::InvalidTimestamp)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let segments = event
            .segments
            .iter()
            .enumerate()
            .map(|(index, segment)| {
                let window = timestamps
                    .get(index + 1)
                    .map_or(NOMINAL_SEGMENT_SECONDS, |next| {
                        (next.ordinal_seconds() - timestamps[index].ordinal_seconds()) as f64
                    });
                SegmentMedia {
                    timestamp: timestamps[index],
                    durations: segment.cameras.map(|_, file| file.map(|_| window)),
                }
            })
            .collect::<Vec<_>>();
        EventTimeline::new(&segments)
    }

    /// Event length in seconds.
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// The segments in order.
    pub fn segments(&self) -> &[TimelineSegment] {
        &self.segments
    }

    /// Where `camera`'s clips sit, in order.
    pub fn placements(&self, camera: CameraId) -> &[Placement] {
        &self.placements[camera]
    }

    /// Whether any camera has a clip anywhere.
    pub fn has_media(&self) -> bool {
        self.placements
            .iter()
            .any(|(_, placements)| !placements.is_empty())
    }

    /// Clamps playback seconds into the event (NaN → 0).
    pub fn clamp(&self, seconds: f64) -> f64 {
        if seconds.is_nan() {
            0.0
        } else {
            seconds.clamp(0.0, self.duration)
        }
    }

    /// The segment and offset of `seconds` (clamped). A boundary belongs to the segment that starts
    /// there; the event end belongs to the last segment. Without segments: segment 0, offset 0.
    pub fn locate(&self, seconds: f64) -> TimelinePosition {
        if self.segments.is_empty() {
            return TimelinePosition {
                segment_index: 0,
                offset: 0.0,
            };
        }
        let seconds = self.clamp(seconds);
        let segment_index = self
            .segments
            .partition_point(|segment| segment.start <= seconds)
            .saturating_sub(1);
        TimelinePosition {
            segment_index,
            offset: seconds - self.segments[segment_index].start,
        }
    }

    /// Event seconds of `position`, clamped into the event.
    pub fn seconds_at(&self, position: TimelinePosition) -> f64 {
        let Some(last) = self.segments.len().checked_sub(1) else {
            return 0.0;
        };
        self.clamp(self.segments[position.segment_index.min(last)].start + position.offset)
    }

    /// The wall-clock moment shown at `seconds`: the segment timestamp plus the whole seconds of the
    /// offset. `None` without segments.
    pub fn wall_clock_at(&self, seconds: f64) -> Option<LocalTimestamp> {
        let position = self.locate(seconds);
        let segment = self.segments.get(position.segment_index)?;
        segment
            .timestamp
            .plus_seconds(position.offset.floor() as i64)
    }

    /// Event seconds of a wall-clock moment (in ordinal seconds, see
    /// [`LocalTimestamp::ordinal_seconds`]); `None` outside the event.
    pub fn seconds_at_wall_clock(&self, ordinal_seconds: i64) -> Option<f64> {
        let origin = self.segments.first()?.timestamp.ordinal_seconds();
        let seconds = (ordinal_seconds - origin) as f64;
        (0.0..=self.duration).contains(&seconds).then_some(seconds)
    }

    /// The clip `camera` shows at `seconds` and the media time inside it; `None` in a gap.
    ///
    /// A clip covers `[start, end)` including its hold, during which the media time stays at the
    /// clip's end (its last frame); at the very end of the event the last clip that ends there still
    /// covers it, so the final frame stays visible.
    pub fn media_time(&self, camera: CameraId, seconds: f64) -> Option<(Placement, f64)> {
        let seconds = self.clamp(seconds);
        let placements = &self.placements[camera];
        let covering = placements
            .iter()
            .find(|placement| placement.start <= seconds && seconds < placement.end())
            .or_else(|| {
                placements.last().filter(|placement| {
                    seconds >= self.duration && placement.end() >= self.duration
                })
            })?;
        Some((*covering, (seconds - covering.start).min(covering.duration)))
    }

    /// The start of the segment containing `seconds`.
    pub fn segment_start_at(&self, seconds: f64) -> f64 {
        self.segments
            .get(self.locate(seconds).segment_index)
            .map_or(0.0, |segment| segment.start)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::contract::{event_input, number_input, run_fixture};

    fn timestamp(value: &str) -> LocalTimestamp {
        parse_filename_local_timestamp(value).expect("valid timestamp")
    }

    fn segment(value: &str, durations: &[(CameraId, f64)]) -> SegmentMedia {
        let mut map = CameraMap::default();
        for (camera, duration) in durations {
            map[*camera] = Some(*duration);
        }
        SegmentMedia {
            timestamp: timestamp(value),
            durations: map,
        }
    }

    fn regular(segment_count: usize) -> EventTimeline {
        let start = timestamp("2026-08-21_12-00-00");
        let segments = (0..segment_count)
            .map(|index| SegmentMedia {
                timestamp: start.plus_seconds(60 * index as i64).expect("timestamp"),
                durations: CameraMap::from_fn(|_| Some(60.0)),
            })
            .collect::<Vec<_>>();
        EventTimeline::new(&segments).expect("timeline")
    }

    #[test]
    fn a_regular_cadence_agrees_with_the_nominal_fixture() {
        run_fixture("playback-timeline", |case| {
            let timeline = regular(case.input("segmentCount").as_u64().expect("count") as usize);
            Some(match case.function.as_str() {
                "eventDuration" => Value::from(timeline.duration()),
                "locatePlaybackTime" => {
                    let position = timeline.locate(number_input(case.input("elapsedSeconds")));
                    json!({"segmentIndex": position.segment_index, "offsetSeconds": position.offset})
                }
                _ => return None,
            })
        });
    }

    #[test]
    fn clips_are_truncated_at_the_next_segment() {
        let timeline = EventTimeline::new(&[
            segment("2026-08-21_12-00-00", &[(CameraId::Front, 60.0)]),
            segment("2026-08-21_12-00-50", &[(CameraId::Front, 60.0)]),
        ])
        .expect("timeline");
        assert_eq!(
            timeline.placements(CameraId::Front),
            &[
                Placement {
                    segment_index: 0,
                    start: 0.0,
                    duration: 50.0,
                    hold: 0.0
                },
                Placement {
                    segment_index: 1,
                    start: 50.0,
                    duration: 60.0,
                    hold: 0.0
                },
            ]
        );
        assert_eq!(timeline.duration(), 110.0);
        assert_eq!(timeline.segments()[0].end, 50.0);
        assert_eq!(timeline.segments()[1].end, 110.0);
    }

    #[test]
    fn real_gaps_stay_on_the_clock() {
        let timeline = EventTimeline::new(&[
            segment("2026-08-21_12-00-00", &[(CameraId::Front, 60.0)]),
            segment("2026-08-21_12-01-30", &[(CameraId::Front, 60.0)]),
        ])
        .expect("timeline");
        assert_eq!(timeline.duration(), 150.0);
        assert_eq!(timeline.media_time(CameraId::Front, 70.0), None);
        assert_eq!(
            timeline
                .media_time(CameraId::Front, 95.0)
                .map(|(_, time)| time),
            Some(5.0)
        );
        assert_eq!(
            timeline.locate(70.0),
            TimelinePosition {
                segment_index: 0,
                offset: 70.0
            }
        );
        assert_eq!(
            timeline.locate(90.0),
            TimelinePosition {
                segment_index: 1,
                offset: 0.0
            }
        );
        assert_eq!(timeline.segment_start_at(120.0), 90.0);
    }

    #[test]
    fn missing_and_short_files_leave_gaps() {
        let timeline = EventTimeline::new(&[
            segment(
                "2026-08-21_12-00-00",
                &[(CameraId::Front, 20.0), (CameraId::Back, 60.0)],
            ),
            segment("2026-08-21_12-01-00", &[(CameraId::Back, 60.0)]),
        ])
        .expect("timeline");
        assert_eq!(timeline.placements(CameraId::Front).len(), 1);
        assert_eq!(
            timeline
                .media_time(CameraId::Front, 19.5)
                .map(|(_, time)| time),
            Some(19.5)
        );
        assert_eq!(timeline.media_time(CameraId::Front, 20.0), None);
        assert_eq!(timeline.media_time(CameraId::Front, 61.0), None);
        assert_eq!(timeline.media_time(CameraId::LeftPillar, 1.0), None);
        assert!(timeline.has_media());
    }

    #[test]
    fn tails_under_a_second_hold_the_last_frame() {
        let timeline = EventTimeline::new(&[
            segment(
                "2026-08-21_12-00-00",
                &[(CameraId::Front, 60.0), (CameraId::Back, 59.97)],
            ),
            segment(
                "2026-08-21_12-01-00",
                &[(CameraId::Front, 60.0), (CameraId::Back, 59.5)],
            ),
        ])
        .expect("timeline");
        let back = timeline.placements(CameraId::Back);
        assert_eq!(back[0].duration, 59.97);
        assert!((back[0].hold - 0.03).abs() < 1e-9);
        assert!((back[0].end() - 60.0).abs() < 1e-9);
        assert!((back[1].hold - 0.5).abs() < 1e-9);
        assert_eq!(timeline.placements(CameraId::Front)[0].hold, 0.0);
        // Inside the hold the clip's last frame stays on screen.
        assert_eq!(
            timeline
                .media_time(CameraId::Back, 59.99)
                .map(|(placement, time)| (placement.segment_index, time)),
            Some((0, 59.97))
        );
        assert_eq!(
            timeline
                .media_time(CameraId::Back, 119.8)
                .map(|(placement, time)| (placement.segment_index, time)),
            Some((1, 59.5))
        );
        assert_eq!(
            timeline
                .media_time(CameraId::Back, 120.0)
                .map(|(placement, time)| (placement.segment_index, time)),
            Some((1, 59.5))
        );
    }

    #[test]
    fn tails_of_a_second_or_more_stay_gaps() {
        let timeline = EventTimeline::new(&[
            segment(
                "2026-08-21_12-00-00",
                &[(CameraId::Front, 60.0), (CameraId::Back, 59.0)],
            ),
            segment("2026-08-21_12-01-00", &[(CameraId::Front, 60.0)]),
        ])
        .expect("timeline");
        let back = timeline.placements(CameraId::Back);
        assert_eq!(back[0].hold, 0.0);
        assert_eq!(back[0].end(), 59.0);
        assert_eq!(timeline.media_time(CameraId::Back, 59.5), None);
    }

    #[test]
    fn the_last_segment_lasts_as_long_as_its_longest_clip() {
        let timeline = EventTimeline::new(&[
            segment("2026-08-21_12-00-00", &[(CameraId::Front, 60.0)]),
            segment(
                "2026-08-21_12-01-00",
                &[(CameraId::Front, 2.0), (CameraId::Back, 59.97)],
            ),
        ])
        .expect("timeline");
        assert!((timeline.duration() - 119.97).abs() < 1e-9);
        let unknown = EventTimeline::new(&[segment("2026-08-21_12-00-00", &[])]).expect("timeline");
        assert_eq!(unknown.duration(), NOMINAL_SEGMENT_SECONDS);
        assert!(!unknown.has_media());
    }

    #[test]
    fn a_last_segment_without_playable_clips_adds_no_length() {
        // Truncated files after a power cut: the probe found nothing in the last segment.
        let timeline = EventTimeline::new(&[
            segment("2026-08-21_12-00-00", &[(CameraId::Front, 59.97)]),
            segment("2026-08-21_12-01-00", &[]),
        ])
        .expect("timeline");
        assert!(
            (timeline.duration() - 60.0).abs() < 1e-9,
            "{}",
            timeline.duration()
        );
        // Short clips end the event where their video ends.
        let short = EventTimeline::new(&[
            segment("2026-08-21_12-00-00", &[(CameraId::Front, 30.0)]),
            segment("2026-08-21_12-01-00", &[]),
        ])
        .expect("timeline");
        assert!(
            (short.duration() - 30.0).abs() < 1e-9,
            "{}",
            short.duration()
        );
    }

    #[test]
    fn the_final_frame_stays_visible_at_the_end() {
        let timeline = regular(2);
        assert_eq!(
            timeline
                .media_time(CameraId::Back, 120.0)
                .map(|(placement, time)| (placement.segment_index, time)),
            Some((1, 60.0))
        );
        assert_eq!(
            timeline
                .media_time(CameraId::Back, 60.0)
                .map(|(placement, time)| (placement.segment_index, time)),
            Some((1, 0.0))
        );
    }

    #[test]
    fn positions_round_trip() {
        let timeline = EventTimeline::new(&[
            segment("2026-08-21_12-00-00", &[(CameraId::Front, 60.0)]),
            segment("2026-08-21_12-01-30", &[(CameraId::Front, 60.0)]),
        ])
        .expect("timeline");
        for seconds in [0.0, 12.5, 89.99, 90.0, 149.0, 150.0] {
            assert_eq!(timeline.seconds_at(timeline.locate(seconds)), seconds);
        }
        assert_eq!(
            timeline.seconds_at(TimelinePosition {
                segment_index: 1,
                offset: 500.0
            }),
            150.0
        );
        assert_eq!(timeline.clamp(f64::NAN), 0.0);
        assert_eq!(timeline.clamp(-4.0), 0.0);
        assert_eq!(timeline.clamp(f64::INFINITY), 150.0);
    }

    #[test]
    fn wall_clock_labels_follow_the_segment_timestamp() {
        let timeline = EventTimeline::new(&[
            segment("2026-08-21_23-59-30", &[(CameraId::Front, 60.0)]),
            segment("2026-08-22_00-00-40", &[(CameraId::Front, 60.0)]),
        ])
        .expect("timeline");
        let label = |seconds| {
            timeline
                .wall_clock_at(seconds)
                .map(|moment| moment.date_time_label())
        };
        assert_eq!(label(15.9).as_deref(), Some("2026-08-21 23:59:45"));
        assert_eq!(label(75.0).as_deref(), Some("2026-08-22 00:00:45"));
        let detection = timestamp("2026-08-22_00-00-45").ordinal_seconds();
        assert_eq!(timeline.seconds_at_wall_clock(detection), Some(75.0));
        assert_eq!(timeline.seconds_at_wall_clock(detection - 3_600), None);
        assert_eq!(timeline.seconds_at_wall_clock(detection + 3_600), None);
    }

    #[test]
    fn segments_must_ascend() {
        let result = EventTimeline::new(&[
            segment("2026-08-21_12-01-00", &[]),
            segment("2026-08-21_12-00-00", &[]),
        ]);
        assert_eq!(result, Err(TimelineError::UnorderedSegments));
    }

    #[test]
    fn an_empty_event_has_no_time() {
        let timeline = EventTimeline::new(&[]).expect("timeline");
        assert_eq!(timeline.duration(), 0.0);
        assert_eq!(
            timeline.locate(30.0),
            TimelinePosition {
                segment_index: 0,
                offset: 0.0
            }
        );
        assert_eq!(timeline.wall_clock_at(0.0), None);
    }

    #[test]
    fn full_windows_from_an_event() {
        let event = event_input(&json!({
            "id": "saved-1", "category": "saved", "timestamp": "2026-08-20_20-10-00",
            "segments": [
                {"timestamp": "2026-08-20_20-10-00", "cameras": ["front", "back"]},
                {"timestamp": "2026-08-20_20-11-30", "cameras": ["front"]},
            ],
        }));
        let timeline = EventTimeline::assuming_full_windows(&event).expect("timeline");
        assert_eq!(timeline.duration(), 150.0);
        assert_eq!(
            timeline.placements(CameraId::Back),
            &[Placement {
                segment_index: 0,
                start: 0.0,
                duration: 90.0,
                hold: 0.0
            }]
        );
        assert_eq!(timeline.placements(CameraId::Front).len(), 2);
        let invalid = event_input(&json!({
            "id": "saved-1", "category": "saved", "timestamp": "x",
            "segments": [{"timestamp": "2026-02-30_00-00-00", "cameras": []}],
        }));
        assert_eq!(
            EventTimeline::assuming_full_windows(&invalid),
            Err(TimelineError::InvalidTimestamp)
        );
    }
}
