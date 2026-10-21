//! The wall-clock event timeline: where every clip sits on the event's time axis.
//!
//! Pure logic. Segment `i` starts at its timestamp offset from segment 0 and is truncated at the next
//! segment's start; missing files, files that cannot be shown and the rest of a short clip's window are
//! gaps. Very short tails (below [`HOLD_TOLERANCE_SECONDS`]) hold the clip's last frame instead, so a
//! 59.97 s clip followed by a segment at 60 s never flashes a missing tile.
//!
//! This is the probed counterpart of `teslacam_core::event_timeline::EventTimeline`: both apply the
//! same placement, truncation and hold rules (a test below checks that they agree).

use crate::input::{CameraId, CameraMap};
use crate::status::FileFault;

/// Tails shorter than this hold the clip's last frame instead of becoming a gap (shared with
/// `teslacam-core`).
pub use teslacam_core::event_timeline::HOLD_TOLERANCE_SECONDS;

/// What probing one clip file found out.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipInfo {
    /// Start of the video track in the file (usually 0).
    pub source_start: f64,
    /// Duration of the video track in seconds (> 0).
    pub duration: f64,
    /// Display size in pixels after the track's preferred transform.
    pub display_width: f64,
    /// See `display_width`.
    pub display_height: f64,
    /// Duration of one frame (from the nominal frame rate; 1/30 when unknown).
    pub frame_duration: f64,
}

/// The probe result of one clip file: shown, or the reason it cannot be shown.
pub type ClipProbe = Result<ClipInfo, FileFault>;

/// One segment with the probe result per camera (`None` = the segment has no file for that camera).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SegmentProbe {
    /// Wall-clock offset from the first segment, in seconds.
    pub start_seconds: f64,
    /// Probe result per camera.
    pub cameras: CameraMap<Option<ClipProbe>>,
}

/// A clip placed on the event time axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedClip {
    /// Index of the segment the clip came from.
    pub segment: usize,
    /// Event time at which the clip starts.
    pub start: f64,
    /// Seconds of the clip's media that are used (after truncation at the next segment).
    pub duration: f64,
    /// Extra seconds the last frame is held to close a tiny tail (0 when there is none).
    pub hold: f64,
    /// The clip's probe data.
    pub info: ClipInfo,
}

impl PlacedClip {
    /// Event time at which the clip (including its hold) ends.
    pub fn end(&self) -> f64 {
        self.start + self.duration + self.hold
    }
}

/// What a camera shows during a span of the event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Coverage {
    /// Video from `clips[clip]` of the camera's timeline.
    Video {
        /// Index into [`CameraTimeline::clips`].
        clip: usize,
    },
    /// No file (missing camera, or the rest of a short clip's window).
    Missing,
    /// A file exists but cannot be shown.
    Fault(FileFault),
}

/// A half-open span `[start, end)` of event time with one coverage.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Span {
    /// Inclusive start.
    pub start: f64,
    /// Exclusive end (the last span of an event also covers `end` itself).
    pub end: f64,
    /// What the camera shows in this span.
    pub coverage: Coverage,
}

/// One camera's clips and coverage over the whole event.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CameraTimeline {
    /// Clips in time order.
    pub clips: Vec<PlacedClip>,
    /// Contiguous spans covering `[0, duration]` in time order (empty when the event has no media).
    pub spans: Vec<Span>,
}

impl CameraTimeline {
    /// Coverage at event time `t`. Boundaries belong to the next span; times outside the event clamp
    /// to the first / last span. `None` only when the event has no media at all.
    pub fn coverage_at(&self, t: f64) -> Option<Coverage> {
        let index = self.spans.partition_point(|span| span.end <= t);
        self.spans
            .get(index.min(self.spans.len().saturating_sub(1)))
            .map(|span| span.coverage)
    }

    /// True when at least one clip of this camera can be shown.
    pub fn has_video(&self) -> bool {
        !self.clips.is_empty()
    }

    /// Spans in which this camera shows no video (missing or faulty), in time order.
    pub fn gaps(&self) -> impl Iterator<Item = &Span> {
        self.spans
            .iter()
            .filter(|span| !matches!(span.coverage, Coverage::Video { .. }))
    }

    /// Display size of the first clip, if any.
    pub fn display_size(&self) -> Option<(f64, f64)> {
        self.clips
            .first()
            .map(|clip| (clip.info.display_width, clip.info.display_height))
    }
}

/// Every camera's placement on the event's wall-clock time axis.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EventTimeline {
    /// Event duration in seconds: the end of the latest clip (0 when nothing can be shown).
    pub duration: f64,
    /// Start of every segment on the event axis.
    pub segment_starts: Vec<f64>,
    /// Per-camera clips and coverage.
    pub cameras: CameraMap<CameraTimeline>,
}

impl EventTimeline {
    /// True when at least one camera has a clip that can be shown.
    pub fn has_playable_media(&self) -> bool {
        self.duration > 0.0
    }

    /// Index of the segment that contains event time `t` (boundaries belong to the next segment).
    pub fn segment_at(&self, t: f64) -> usize {
        self.segment_starts
            .partition_point(|start| *start <= t)
            .saturating_sub(1)
    }
}

/// Places every probed clip on the wall-clock event axis (see the module docs).
pub fn plan_event_timeline(segments: &[SegmentProbe]) -> EventTimeline {
    let origin = segments
        .first()
        .map_or(0.0, |segment| segment.start_seconds);
    let starts: Vec<f64> = segments
        .iter()
        .map(|segment| (segment.start_seconds - origin).max(0.0))
        .collect();
    // The last window lasts as long as the longest clip of the last segment.
    let last_window_end = segments
        .last()
        .zip(starts.last())
        .map_or(0.0, |(segment, start)| start + longest(segment));
    let window_end = |index: usize| starts.get(index + 1).copied().unwrap_or(last_window_end);

    let mut cameras = CameraMap::from_fn(|_| CameraTimeline::default());
    for camera in CameraId::ALL {
        let timeline = &mut cameras[camera];
        for (index, segment) in segments.iter().enumerate() {
            let (start, end) = (starts[index], window_end(index).max(starts[index]));
            match segment.cameras[camera] {
                None => push_span(&mut timeline.spans, start, end, Coverage::Missing),
                Some(Err(fault)) => {
                    push_span(&mut timeline.spans, start, end, Coverage::Fault(fault))
                }
                Some(Ok(info)) => {
                    let duration = info.duration.min(end - start).max(0.0);
                    if duration <= 0.0 {
                        continue;
                    }
                    let tail = end - start - duration;
                    let hold = if tail > 0.0 && tail < HOLD_TOLERANCE_SECONDS {
                        tail
                    } else {
                        0.0
                    };
                    let clip = PlacedClip {
                        segment: index,
                        start,
                        duration,
                        hold,
                        info,
                    };
                    timeline.clips.push(clip);
                    let video = Coverage::Video {
                        clip: timeline.clips.len() - 1,
                    };
                    push_span(&mut timeline.spans, start, clip.end(), video);
                    push_span(&mut timeline.spans, clip.end(), end, Coverage::Missing);
                }
            }
        }
    }
    let duration = CameraId::ALL
        .iter()
        .filter_map(|camera| cameras[*camera].clips.last().map(PlacedClip::end))
        .fold(0.0, f64::max);
    for camera in CameraId::ALL {
        clip_spans(&mut cameras[camera].spans, duration);
    }
    EventTimeline {
        duration,
        segment_starts: starts,
        cameras,
    }
}

fn longest(segment: &SegmentProbe) -> f64 {
    segment
        .cameras
        .iter()
        .filter_map(|(_, probe)| probe.as_ref().and_then(|probe| probe.as_ref().ok()))
        .map(|info| info.duration)
        .fold(0.0, f64::max)
}

fn push_span(spans: &mut Vec<Span>, start: f64, end: f64, coverage: Coverage) {
    if end <= start {
        return;
    }
    if let Some(last) = spans.last_mut()
        && last.coverage == coverage
        && !matches!(coverage, Coverage::Video { .. })
        && (last.end - start).abs() < 1e-9
    {
        last.end = end;
        return;
    }
    spans.push(Span {
        start,
        end,
        coverage,
    });
}

/// Drops everything after the event's end and closes any hole before it with `Missing`.
fn clip_spans(spans: &mut Vec<Span>, duration: f64) {
    if duration <= 0.0 {
        spans.clear();
        return;
    }
    spans.retain(|span| span.start < duration);
    if let Some(last) = spans.last_mut() {
        last.end = last.end.min(duration);
    }
    let end = spans.last().map_or(0.0, |span| span.end);
    push_span(spans, end, duration, Coverage::Missing);
}

#[cfg(test)]
mod tests {
    use super::*;

    const FD: f64 = 1.0 / 36.0;

    fn info(duration: f64) -> ClipInfo {
        ClipInfo {
            source_start: 0.0,
            duration,
            display_width: 1280.0,
            display_height: 960.0,
            frame_duration: FD,
        }
    }

    fn segment(start: f64, f: impl Fn(CameraId) -> Option<ClipProbe>) -> SegmentProbe {
        SegmentProbe {
            start_seconds: start,
            cameras: CameraMap::from_fn(f),
        }
    }

    fn all(start: f64, duration: f64) -> SegmentProbe {
        segment(start, |_| Some(Ok(info(duration))))
    }

    fn span(start: f64, end: f64, coverage: Coverage) -> Span {
        Span {
            start,
            end,
            coverage,
        }
    }

    #[test]
    fn full_segments_are_placed_back_to_back() {
        let timeline = plan_event_timeline(&[all(0.0, 60.0), all(60.0, 60.0)]);
        assert_eq!(timeline.duration, 120.0);
        assert_eq!(timeline.segment_starts, [0.0, 60.0]);
        for camera in CameraId::ALL {
            let camera = &timeline.cameras[camera];
            assert_eq!(camera.clips.len(), 2);
            assert_eq!(
                (
                    camera.clips[1].start,
                    camera.clips[1].duration,
                    camera.clips[1].hold
                ),
                (60.0, 60.0, 0.0)
            );
            assert_eq!(
                camera.spans,
                [
                    span(0.0, 60.0, Coverage::Video { clip: 0 }),
                    span(60.0, 120.0, Coverage::Video { clip: 1 })
                ]
            );
        }
    }

    #[test]
    fn long_clips_are_truncated_at_the_next_segment() {
        let timeline = plan_event_timeline(&[all(0.0, 61.5), all(60.0, 60.0)]);
        let front = &timeline.cameras[CameraId::Front];
        assert_eq!(front.clips[0].duration, 60.0);
        assert_eq!(front.clips[0].end(), 60.0);
        assert_eq!(timeline.duration, 120.0);
    }

    #[test]
    fn short_clips_leave_a_missing_gap_until_the_next_segment() {
        let short = segment(20.0, |camera| {
            Some(Ok(info(if camera == CameraId::Front { 8.0 } else { 20.0 })))
        });
        let timeline = plan_event_timeline(&[all(0.0, 20.0), short, all(40.0, 20.0)]);
        let front = &timeline.cameras[CameraId::Front];
        assert_eq!(
            front.spans,
            [
                span(0.0, 20.0, Coverage::Video { clip: 0 }),
                span(20.0, 28.0, Coverage::Video { clip: 1 }),
                span(28.0, 40.0, Coverage::Missing),
                span(40.0, 60.0, Coverage::Video { clip: 2 }),
            ]
        );
        assert_eq!(front.gaps().count(), 1);
    }

    #[test]
    fn tiny_tails_hold_the_last_frame() {
        let timeline = plan_event_timeline(&[all(0.0, 59.97), all(60.0, 60.0)]);
        let clip = timeline.cameras[CameraId::Back].clips[0];
        assert_eq!(clip.duration, 59.97);
        assert!((clip.hold - 0.03).abs() < 1e-9);
        assert_eq!(timeline.cameras[CameraId::Back].gaps().count(), 0);
    }

    #[test]
    fn missing_and_faulty_files_become_gaps() {
        let first = segment(0.0, |camera| match camera {
            CameraId::RightRepeater => None,
            CameraId::LeftPillar => Some(Err(FileFault::Corrupt)),
            _ => Some(Ok(info(30.0))),
        });
        let timeline = plan_event_timeline(&[first, all(30.0, 30.0)]);
        assert_eq!(
            timeline.cameras[CameraId::RightRepeater].spans,
            [
                span(0.0, 30.0, Coverage::Missing),
                span(30.0, 60.0, Coverage::Video { clip: 0 })
            ]
        );
        assert_eq!(
            timeline.cameras[CameraId::LeftPillar].spans[0],
            span(0.0, 30.0, Coverage::Fault(FileFault::Corrupt))
        );
        assert_eq!(
            timeline.cameras[CameraId::LeftPillar].coverage_at(10.0),
            Some(Coverage::Fault(FileFault::Corrupt))
        );
    }

    #[test]
    fn a_camera_without_any_file_is_missing_for_the_whole_event() {
        let probe = |start| {
            segment(start, |camera| {
                (camera != CameraId::Back).then_some(Ok(info(20.0)))
            })
        };
        let timeline = plan_event_timeline(&[probe(0.0), probe(20.0)]);
        let back = &timeline.cameras[CameraId::Back];
        assert!(!back.has_video());
        assert_eq!(back.spans, [span(0.0, 40.0, Coverage::Missing)]);
        assert_eq!(back.display_size(), None);
    }

    #[test]
    fn the_last_window_lasts_as_long_as_its_longest_clip() {
        let last = segment(60.0, |camera| {
            Some(Ok(info(if camera == CameraId::Front {
                10.0
            } else {
                30.0
            })))
        });
        let timeline = plan_event_timeline(&[all(0.0, 60.0), last]);
        assert_eq!(timeline.duration, 90.0);
        assert_eq!(
            timeline.cameras[CameraId::Front].spans.last(),
            Some(&span(70.0, 90.0, Coverage::Missing))
        );
        assert_eq!(
            timeline.cameras[CameraId::Back].spans.last(),
            Some(&span(60.0, 90.0, Coverage::Video { clip: 1 }))
        );
    }

    #[test]
    fn real_gaps_between_segments_stay_on_the_wall_clock() {
        let timeline = plan_event_timeline(&[all(0.0, 60.0), all(85.0, 60.0)]);
        assert_eq!(timeline.duration, 145.0);
        assert_eq!(
            timeline.cameras[CameraId::Front].spans,
            [
                span(0.0, 60.0, Coverage::Video { clip: 0 }),
                span(60.0, 85.0, Coverage::Missing),
                span(85.0, 145.0, Coverage::Video { clip: 1 }),
            ]
        );
        assert_eq!(timeline.segment_at(84.9), 0);
        assert_eq!(timeline.segment_at(85.0), 1);
    }

    #[test]
    fn offsets_are_relative_to_the_first_segment() {
        let timeline = plan_event_timeline(&[all(5.0, 60.0), all(65.0, 60.0)]);
        assert_eq!(timeline.segment_starts, [0.0, 60.0]);
        assert_eq!(timeline.duration, 120.0);
    }

    #[test]
    fn an_event_without_playable_media_has_no_duration_and_no_spans() {
        let broken = segment(0.0, |_| Some(Err(FileFault::Unsupported)));
        let timeline = plan_event_timeline(&[broken]);
        assert!(!timeline.has_playable_media());
        assert_eq!(timeline.duration, 0.0);
        assert_eq!(timeline.cameras[CameraId::Front].coverage_at(0.0), None);
        assert!(!plan_event_timeline(&[]).has_playable_media());
    }

    #[test]
    fn faults_in_the_last_segment_end_with_the_event() {
        let last = segment(60.0, |camera| {
            Some(if camera == CameraId::Front {
                Err(FileFault::Corrupt)
            } else {
                Ok(info(60.0))
            })
        });
        let timeline = plan_event_timeline(&[all(0.0, 60.0), last]);
        assert_eq!(
            timeline.cameras[CameraId::Front].spans.last(),
            Some(&span(60.0, 120.0, Coverage::Fault(FileFault::Corrupt)))
        );
    }

    #[test]
    fn coverage_lookup_puts_boundaries_into_the_next_span_and_clamps() {
        let timeline = plan_event_timeline(&[all(0.0, 60.0), all(60.0, 60.0)]);
        let front = &timeline.cameras[CameraId::Front];
        assert_eq!(front.coverage_at(-1.0), Some(Coverage::Video { clip: 0 }));
        assert_eq!(front.coverage_at(59.999), Some(Coverage::Video { clip: 0 }));
        assert_eq!(front.coverage_at(60.0), Some(Coverage::Video { clip: 1 }));
        assert_eq!(front.coverage_at(120.0), Some(Coverage::Video { clip: 1 }));
        assert_eq!(front.coverage_at(500.0), Some(Coverage::Video { clip: 1 }));
    }

    /// Builds the same event for both crates: segment offsets in whole seconds and clip lengths per
    /// camera (`None` = no file).
    fn agree(segments: &[(i64, [Option<f64>; 6])]) {
        use teslacam_core::event_timeline::{EventTimeline as CoreTimeline, SegmentMedia};
        use teslacam_core::timestamp::parse_filename_local_timestamp;

        let origin = parse_filename_local_timestamp("2026-08-21_12-00-00").expect("timestamp");
        let core = CoreTimeline::new(
            &segments
                .iter()
                .map(|(offset, lengths)| SegmentMedia {
                    timestamp: origin.plus_seconds(*offset).expect("timestamp"),
                    durations: CameraMap::from_fn(|camera| lengths[camera.index()]),
                })
                .collect::<Vec<_>>(),
        )
        .expect("core timeline");
        let media = plan_event_timeline(
            &segments
                .iter()
                .map(|(offset, lengths)| {
                    segment(*offset as f64, |camera| {
                        lengths[camera.index()].map(|length| Ok(info(length)))
                    })
                })
                .collect::<Vec<_>>(),
        );
        assert!((core.duration() - media.duration).abs() < 1e-9, "duration");
        for camera in CameraId::ALL {
            let core_clips: Vec<_> = core
                .placements(camera)
                .iter()
                .map(|p| (p.segment_index, p.start, p.duration, p.hold))
                .collect();
            let media_clips: Vec<_> = media.cameras[camera]
                .clips
                .iter()
                .map(|c| (c.segment, c.start, c.duration, c.hold))
                .collect();
            assert_eq!(core_clips.len(), media_clips.len(), "{camera:?}");
            for (core_clip, media_clip) in core_clips.iter().zip(&media_clips) {
                assert_eq!((core_clip.0, core_clip.1), (media_clip.0, media_clip.1));
                assert!(
                    (core_clip.2 - media_clip.2).abs() < 1e-9,
                    "{camera:?} duration"
                );
                assert!((core_clip.3 - media_clip.3).abs() < 1e-9, "{camera:?} hold");
            }
        }
    }

    #[test]
    fn the_core_timeline_places_clips_the_same_way() {
        let full = [Some(60.0); 6];
        agree(&[(0, full), (60, full)]);
        // Tiny tails hold, longer ones are gaps, missing files are gaps.
        agree(&[
            (
                0,
                [
                    Some(59.97),
                    Some(60.0),
                    Some(59.0),
                    None,
                    Some(61.5),
                    Some(60.0),
                ],
            ),
            (
                60,
                [
                    Some(60.0),
                    Some(59.5),
                    Some(30.0),
                    Some(60.0),
                    None,
                    Some(58.0),
                ],
            ),
        ]);
        // Real gaps between segments and a short last segment.
        agree(&[
            (0, full),
            (85, [Some(20.0), Some(19.6), None, None, None, Some(5.0)]),
        ]);
    }

    #[test]
    fn segments_sharing_a_start_keep_only_the_later_one() {
        let timeline = plan_event_timeline(&[all(0.0, 60.0), all(0.0, 30.0)]);
        let front = &timeline.cameras[CameraId::Front];
        assert_eq!(front.clips.len(), 1);
        assert_eq!(front.clips[0].segment, 1);
        assert_eq!(timeline.duration, 30.0);
    }
}
