//! What the composite export draws where and when. Pure logic.

use crate::{
    input::{CameraId, CameraMap},
    layout::{ExportLayout, Rect},
    timeline::{CameraTimeline, Coverage, EventTimeline},
};

/// Discrete opacity keyframes in the form `CAKeyframeAnimation` expects for
/// `kCAAnimationDiscrete`: `values[i]` holds from `key_times[i]` to `key_times[i + 1]`, so there is one
/// more key time than values. Key times are fractions of the animation duration (0…1).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Keyframes {
    pub key_times: Vec<f64>,
    pub values: Vec<f64>,
}

/// When a camera's missing tile (`記録なし`) is visible.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MissingTile {
    /// The camera has video for the whole event.
    Never,
    /// The camera has no video at all.
    Always,
    /// Visible only in some ranges.
    During(Keyframes),
}

/// One camera in the export frame.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CameraPlan {
    /// The fitted video rect: the rounded hole in the overlay and the missing tile's frame.
    pub hole: Rect,
    /// True when at least one clip of the camera is composed.
    pub has_video: bool,
    pub missing: MissingTile,
}

/// The whole export.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExportPlan {
    pub duration: f64,
    /// Frames AVFoundation renders at the layout's frame rate.
    pub frame_count: u64,
    pub cameras: CameraMap<CameraPlan>,
}

/// Plans the export of `timeline`; `None` when the event has nothing to show.
pub(crate) fn plan_export(timeline: &EventTimeline, layout: &ExportLayout) -> Option<ExportPlan> {
    if !timeline.has_playable_media() {
        return None;
    }
    let duration = timeline.duration;
    let cameras = CameraMap::from_fn(|camera: CameraId| {
        let camera_timeline = &timeline.cameras[camera];
        CameraPlan {
            hole: layout.video_rect(camera, camera_timeline.display_size()),
            has_video: camera_timeline.has_video(),
            missing: missing_tile(camera_timeline, duration),
        }
    });
    let frame_count = (duration * f64::from(layout.fps) - 1e-6).ceil().max(1.0) as u64;
    Some(ExportPlan {
        duration,
        frame_count,
        cameras,
    })
}

/// The missing tile schedule of one camera.
pub(crate) fn missing_tile(timeline: &CameraTimeline, duration: f64) -> MissingTile {
    let mut gaps: Vec<(f64, f64)> = Vec::new();
    for span in &timeline.spans {
        if matches!(span.coverage, Coverage::Video { .. }) {
            continue;
        }
        match gaps.last_mut() {
            Some(last) if (last.1 - span.start).abs() < 1e-9 => last.1 = span.end,
            _ => gaps.push((span.start, span.end)),
        }
    }
    if timeline.spans.is_empty() || !timeline.has_video() {
        return MissingTile::Always;
    }
    if gaps.is_empty() {
        return MissingTile::Never;
    }
    let mut key_times = vec![0.0];
    let mut values = Vec::new();
    let mut cursor = 0.0;
    for (start, end) in gaps {
        if start > cursor {
            values.push(0.0);
            key_times.push(start / duration);
        }
        values.push(1.0);
        key_times.push(end / duration);
        cursor = end;
    }
    if cursor < duration {
        values.push(0.0);
        key_times.push(1.0);
    }
    if let Some(last) = key_times.last_mut() {
        *last = 1.0;
    }
    MissingTile::During(Keyframes { key_times, values })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        status::FileFault,
        timeline::{ClipInfo, ClipProbe, SegmentProbe, plan_event_timeline},
    };

    fn info(duration: f64) -> ClipInfo {
        ClipInfo {
            source_start: 0.0,
            duration,
            display_width: 1280.0,
            display_height: 960.0,
            frame_duration: 1.0 / 36.0,
        }
    }

    fn segment(start: f64, f: impl Fn(CameraId) -> Option<ClipProbe>) -> SegmentProbe {
        SegmentProbe {
            start_seconds: start,
            cameras: CameraMap::from_fn(f),
        }
    }

    /// The fixture "gaps" event: no right repeater, no back clip in segment 1, an 8 s front clip.
    fn gaps_timeline() -> EventTimeline {
        plan_event_timeline(&[
            segment(0.0, |c| {
                (c != CameraId::RightRepeater).then_some(Ok(info(20.0)))
            }),
            segment(20.0, |c| match c {
                CameraId::RightRepeater | CameraId::Back => None,
                CameraId::Front => Some(Ok(info(8.0))),
                _ => Some(Ok(info(20.0))),
            }),
            segment(40.0, |c| {
                (c != CameraId::RightRepeater).then_some(Ok(info(20.0)))
            }),
        ])
    }

    fn keyframes(key_times: &[f64], values: &[f64]) -> MissingTile {
        MissingTile::During(Keyframes {
            key_times: key_times.to_vec(),
            values: values.to_vec(),
        })
    }

    #[test]
    fn missing_tiles_follow_the_gaps() {
        let timeline = gaps_timeline();
        let plan = plan_export(&timeline, &ExportLayout::contract()).expect("playable");
        assert_eq!(plan.duration, 60.0);
        assert_eq!(plan.frame_count, 1800);
        assert_eq!(
            plan.cameras[CameraId::LeftPillar].missing,
            MissingTile::Never
        );
        assert_eq!(
            plan.cameras[CameraId::RightRepeater].missing,
            MissingTile::Always
        );
        assert!(!plan.cameras[CameraId::RightRepeater].has_video);
        assert_eq!(
            plan.cameras[CameraId::Back].missing,
            keyframes(&[0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0], &[0.0, 1.0, 0.0])
        );
        assert_eq!(
            plan.cameras[CameraId::Front].missing,
            keyframes(&[0.0, 28.0 / 60.0, 2.0 / 3.0, 1.0], &[0.0, 1.0, 0.0])
        );
    }

    #[test]
    fn holes_are_the_fitted_video_rects() {
        let plan = plan_export(&gaps_timeline(), &ExportLayout::contract()).unwrap();
        let layout = ExportLayout::contract();
        for camera in CameraId::ALL {
            assert_eq!(plan.cameras[camera].hole, layout.slots[camera]);
        }
    }

    #[test]
    fn gaps_at_the_edges_and_adjacent_faults_merge() {
        let timeline = plan_event_timeline(&[
            segment(0.0, |c| {
                if c == CameraId::Front {
                    Some(Err(FileFault::Corrupt))
                } else {
                    Some(Ok(info(10.0)))
                }
            }),
            segment(10.0, |c| (c != CameraId::Front).then_some(Ok(info(10.0)))),
            segment(20.0, |_| Some(Ok(info(10.0)))),
            segment(30.0, |c| (c != CameraId::Back).then_some(Ok(info(10.0)))),
        ]);
        assert_eq!(
            missing_tile(&timeline.cameras[CameraId::Front], 40.0),
            keyframes(&[0.0, 0.5, 1.0], &[1.0, 0.0])
        );
        assert_eq!(
            missing_tile(&timeline.cameras[CameraId::Back], 40.0),
            keyframes(&[0.0, 0.75, 1.0], &[0.0, 1.0])
        );
    }

    #[test]
    fn events_without_media_cannot_be_exported() {
        let timeline = plan_event_timeline(&[segment(0.0, |_| Some(Err(FileFault::Unsupported)))]);
        assert_eq!(plan_export(&timeline, &ExportLayout::contract()), None);
    }

    #[test]
    fn frame_counts_round_up_partial_frames() {
        let timeline = plan_event_timeline(&[segment(0.0, |_| Some(Ok(info(59.97))))]);
        assert_eq!(
            plan_export(&timeline, &ExportLayout::contract())
                .unwrap()
                .frame_count,
            1800
        );
        let timeline = plan_event_timeline(&[segment(0.0, |_| Some(Ok(info(2.0))))]);
        assert_eq!(
            plan_export(&timeline, &ExportLayout::contract())
                .unwrap()
                .frame_count,
            60
        );
    }
}
