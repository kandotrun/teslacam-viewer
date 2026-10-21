//! The composite export frame and plan (pure).
//!
//! The frame geometry comes from `packages/contract/composite-layout.json` (shared with the web
//! exporter); colours are token names resolved through [`Theme`]. The plan follows the Mac's
//! wall-clock [`EventTimeline`], so the exported video shows what playback shows: segments in order,
//! each camera at its real offset, `記録なし` where a camera has no footage.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::camera::{CameraId, CameraMap};
use crate::copy;
use crate::event_timeline::EventTimeline;
use crate::theme::{Rgba, Theme};

/// The layout file, embedded at build time.
pub const COMPOSITE_LAYOUT_JSON: &str =
    include_str!("../../../../../packages/contract/composite-layout.json");

/// A rectangle in output pixels.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rect {
    /// Left.
    pub x: f64,
    /// Top.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

/// Fits `source_width × source_height` inside `destination` without cropping, centred, without
/// rounding. A source without a positive size uses the destination itself.
pub fn contain_rect(source_width: f64, source_height: f64, destination: Rect) -> Rect {
    if !(source_width > 0.0 && source_height > 0.0) {
        return destination;
    }
    let scale = (destination.width / source_width).min(destination.height / source_height);
    let (width, height) = (source_width * scale, source_height * scale);
    Rect {
        x: destination.x + (destination.width - width) / 2.0,
        y: destination.y + (destination.height - height) / 2.0,
        width,
        height,
    }
}

/// The camera name pill drawn over each video.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelStyle {
    /// Distance from the video's left and bottom edges.
    pub inset: f64,
    /// Pill height.
    pub height: f64,
    /// Horizontal padding inside the pill.
    pub padding_inline: f64,
    /// Font size (UI font).
    pub font_size: f64,
    /// Font weight.
    pub font_weight: u16,
    /// Pill colour.
    pub scrim: Rgba,
    /// Text colour.
    pub text: Rgba,
}

/// How a camera without footage is drawn.
#[derive(Debug, Clone, PartialEq)]
pub struct MissingStyle {
    /// Fill of the slot.
    pub fill: Rgba,
    /// Colour of the centred message.
    pub text: Rgba,
    /// The centred message (`記録なし`).
    pub message: String,
}

/// A label pill and where its text starts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelPill {
    /// The pill.
    pub rect: Rect,
    /// Left end of the text's alphabetic baseline.
    pub text_origin: (f64, f64),
}

/// Baseline of the label text below the pill's top edge (13 px text in a 24 px pill).
pub const LABEL_BASELINE: f64 = 17.0;

/// The 1280 × 720 export frame.
#[derive(Debug, Clone, PartialEq)]
pub struct CompositeLayout {
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
    /// Output frame rate.
    pub fps: u32,
    /// Background colour.
    pub background: Rgba,
    /// The slot of each camera.
    pub slots: CameraMap<Rect>,
    /// The car image box.
    pub vehicle: Rect,
    /// Corner radius of each fitted video.
    pub video_corner_radius: f64,
    /// Camera name pills.
    pub label: LabelStyle,
    /// Cameras without footage.
    pub missing: MissingStyle,
    /// Colour of the neutral field-of-view wedges.
    pub wedge_color: Rgba,
    /// Camera names written into the video.
    pub labels: CameraMap<String>,
}

/// The layout file does not match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayoutError(String);

impl fmt::Display for LayoutError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "composite-layout.json: {}", self.0)
    }
}

impl std::error::Error for LayoutError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawLabel {
    inset: f64,
    height: f64,
    padding_inline: f64,
    font_size: f64,
    font_weight: u16,
    scrim: String,
    text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawMissing {
    fill: String,
    text: String,
    message: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawWedges {
    color: String,
    mounts_from: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct RawLayout {
    #[serde(rename = "$comment")]
    _comment: Option<String>,
    width: u32,
    height: u32,
    fps: u32,
    background: String,
    slots: BTreeMap<String, Rect>,
    vehicle: Rect,
    video_corner_radius: f64,
    label: RawLabel,
    missing: RawMissing,
    wedges: RawWedges,
    labels: BTreeMap<String, String>,
}

fn per_camera<T: Clone>(
    mut map: BTreeMap<String, T>,
    what: &str,
) -> Result<CameraMap<T>, LayoutError> {
    let mut values = Vec::new();
    for camera in CameraId::ALL {
        values.push(
            map.remove(camera.id())
                .ok_or_else(|| LayoutError(format!("missing {what} {}", camera.id())))?,
        );
    }
    if let Some(unknown) = map.keys().next() {
        return Err(LayoutError(format!("unknown {what} {unknown}")));
    }
    let mut values = values.into_iter();
    Ok(CameraMap::from_fn(|_| values.next().expect("six values")))
}

impl CompositeLayout {
    /// Parses a layout file strictly, resolving colour tokens through `theme`.
    pub fn parse(json: &str, theme: &Theme) -> Result<CompositeLayout, LayoutError> {
        let raw: RawLayout =
            serde_json::from_str(json).map_err(|error| LayoutError(error.to_string()))?;
        let color = |token: &str| {
            theme
                .colors
                .get(token)
                .ok_or_else(|| LayoutError(format!("unknown colour token {token}")))
        };
        if raw.wedges.mounts_from != "design-tokens.json#/stage/mounts" {
            return Err(LayoutError(format!(
                "unexpected mounts source {}",
                raw.wedges.mounts_from
            )));
        }
        let labels = per_camera(raw.labels, "label")?;
        for (camera, label) in labels.iter() {
            if label != copy::camera_label(camera) {
                return Err(LayoutError(format!(
                    "label of {} is {label}, not {}",
                    camera.id(),
                    copy::camera_label(camera)
                )));
            }
        }
        Ok(CompositeLayout {
            width: raw.width,
            height: raw.height,
            fps: raw.fps,
            background: color(&raw.background)?,
            slots: per_camera(raw.slots, "slot")?,
            vehicle: raw.vehicle,
            video_corner_radius: raw.video_corner_radius,
            label: LabelStyle {
                inset: raw.label.inset,
                height: raw.label.height,
                padding_inline: raw.label.padding_inline,
                font_size: raw.label.font_size,
                font_weight: raw.label.font_weight,
                scrim: color(&raw.label.scrim)?,
                text: color(&raw.label.text)?,
            },
            missing: MissingStyle {
                fill: color(&raw.missing.fill)?,
                text: color(&raw.missing.text)?,
                message: raw.missing.message,
            },
            wedge_color: color(&raw.wedges.color)?,
            labels,
        })
    }

    /// The embedded layout with the embedded theme.
    pub fn embedded() -> &'static CompositeLayout {
        static LAYOUT: OnceLock<CompositeLayout> = OnceLock::new();
        LAYOUT.get_or_init(|| {
            CompositeLayout::parse(COMPOSITE_LAYOUT_JSON, Theme::embedded())
                .expect("embedded layout is valid")
        })
    }

    /// Where `camera`'s video is drawn: contain-fitted into its slot, or the whole slot when the
    /// camera has no footage (`None`).
    pub fn video_rect(&self, camera: CameraId, source_size: Option<(f64, f64)>) -> Rect {
        let slot = self.slots[camera];
        source_size.map_or(slot, |(width, height)| contain_rect(width, height, slot))
    }

    /// The car image contain-fitted into its box.
    pub fn vehicle_rect(&self, image_width: f64, image_height: f64) -> Rect {
        contain_rect(image_width, image_height, self.vehicle)
    }

    /// The name pill at the bottom-left of `video`, for a label whose text is `text_advance` wide.
    pub fn label_pill(&self, video: Rect, text_advance: f64) -> LabelPill {
        let style = &self.label;
        let rect = Rect {
            x: video.x + style.inset,
            y: video.y + video.height - style.inset - style.height,
            width: text_advance.max(0.0).ceil() + 2.0 * style.padding_inline,
            height: style.height,
        };
        LabelPill {
            rect,
            text_origin: (rect.x + style.padding_inline, rect.y + LABEL_BASELINE),
        }
    }
}

/// One segment window of the export.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportSegment {
    /// Segment index.
    pub index: usize,
    /// Output seconds at which the window starts.
    pub start: f64,
    /// Output seconds at which it ends.
    pub end: f64,
    /// Cameras with a clip in this segment, canonical order.
    pub present: Vec<CameraId>,
    /// Cameras drawn as `記録なし` for the whole window, canonical order.
    pub missing: Vec<CameraId>,
}

/// Progress of a running export, ready for display.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportProgress {
    /// Fraction 0–1.
    pub fraction: f64,
    /// Floor of the percentage, 0–100.
    pub percent: u8,
    /// Output seconds written.
    pub elapsed_seconds: f64,
    /// `MM:SS / MM:SS`.
    pub clock: String,
    /// `書き出し中 42%`.
    pub label: String,
}

/// Why nothing can be exported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportPlanError {
    /// No camera has footage anywhere in the event (`再生可能な動画がありません。`).
    NoPlayableMedia,
}

/// What the exporter writes.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportPlan {
    /// Output duration in seconds (the event timeline's duration).
    pub duration: f64,
    /// Output frames at the layout's frame rate (`ceil(duration × fps)`).
    pub frame_count: u64,
    /// Segment windows in order.
    pub segments: Vec<ExportSegment>,
    gaps: CameraMap<Vec<(f64, f64)>>,
}

impl ExportPlan {
    /// Plans the export of `timeline` at `fps` frames per second.
    pub fn new(timeline: &EventTimeline, fps: u32) -> Result<ExportPlan, ExportPlanError> {
        if !timeline.has_media() {
            return Err(ExportPlanError::NoPlayableMedia);
        }
        let duration = timeline.duration();
        let segments = timeline
            .segments()
            .iter()
            .enumerate()
            .map(|(index, segment)| {
                let present = CameraId::ALL
                    .into_iter()
                    .filter(|camera| {
                        timeline
                            .placements(*camera)
                            .iter()
                            .any(|placement| placement.segment_index == index)
                    })
                    .collect::<Vec<_>>();
                ExportSegment {
                    index,
                    start: segment.start,
                    end: segment.end,
                    missing: CameraId::ALL
                        .into_iter()
                        .filter(|camera| !present.contains(camera))
                        .collect(),
                    present,
                }
            })
            .collect();
        let gaps = CameraMap::from_fn(|camera| {
            let mut gaps = Vec::new();
            let mut cursor = 0.0;
            for placement in timeline.placements(camera) {
                if placement.start > cursor {
                    gaps.push((cursor, placement.start));
                }
                cursor = placement.end();
            }
            if cursor < duration {
                gaps.push((cursor, duration));
            }
            gaps
        });
        Ok(ExportPlan {
            duration,
            frame_count: (duration * f64::from(fps)).ceil() as u64,
            segments,
            gaps,
        })
    }

    /// The spans `[start, end)` where `camera` has no footage (drawn as `記録なし`).
    pub fn camera_gaps(&self, camera: CameraId) -> &[(f64, f64)] {
        &self.gaps[camera]
    }

    /// The cameras without footage at output second `seconds`, canonical order.
    pub fn missing_at(&self, seconds: f64) -> Vec<CameraId> {
        CameraId::ALL
            .into_iter()
            .filter(|camera| {
                self.gaps[*camera]
                    .iter()
                    .any(|(start, end)| *start <= seconds && seconds < *end)
            })
            .collect()
    }

    /// Display values for an export `fraction` (clamped; NaN counts as 0).
    pub fn progress(&self, fraction: f64) -> ExportProgress {
        let fraction = if fraction.is_nan() {
            0.0
        } else {
            fraction.clamp(0.0, 1.0)
        };
        ExportProgress {
            fraction,
            percent: copy::progress_percent(fraction),
            elapsed_seconds: fraction * self.duration,
            clock: copy::export_progress_clock(fraction, self.duration),
            label: copy::export_running_text(fraction),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event_timeline::SegmentMedia;
    use crate::timestamp::parse_filename_local_timestamp;

    fn assert_rect(actual: Rect, expected: (f64, f64, f64, f64)) {
        let close = |left: f64, right: f64| (left - right).abs() < 1e-6;
        assert!(
            close(actual.x, expected.0)
                && close(actual.y, expected.1)
                && close(actual.width, expected.2)
                && close(actual.height, expected.3),
            "{actual:?} != {expected:?}"
        );
    }

    #[test]
    fn the_embedded_layout_matches_the_design() {
        let layout = CompositeLayout::embedded();
        let theme = Theme::embedded();
        assert_eq!((layout.width, layout.height, layout.fps), (1280, 720, 30));
        assert_eq!(layout.background, theme.colors.floor);
        let expected = [
            (CameraId::Front, (464.0, 12.0, 352.0, 264.0)),
            (CameraId::LeftPillar, (56.0, 112.0, 352.0, 264.0)),
            (CameraId::RightPillar, (872.0, 112.0, 352.0, 264.0)),
            (CameraId::LeftRepeater, (56.0, 444.0, 352.0, 264.0)),
            (CameraId::RightRepeater, (872.0, 444.0, 352.0, 264.0)),
            (CameraId::Back, (480.0, 456.0, 320.0, 240.0)),
        ];
        for (camera, rect) in expected {
            assert_rect(layout.slots[camera], rect);
            assert_eq!(layout.labels[camera], copy::camera_label(camera));
        }
        assert_rect(layout.vehicle, (598.0, 285.0, 84.0, 168.0));
        assert_eq!(layout.label.scrim, theme.colors.video_label_scrim);
        assert_eq!(layout.label.text, theme.colors.video_text);
        assert_eq!(layout.missing.fill, theme.colors.video_missing_a);
        assert_eq!(layout.missing.message, copy::EXPORT_MISSING_CAMERA);
        assert_eq!(layout.wedge_color, theme.colors.wedge);
        assert_eq!(
            (layout.label.font_size, layout.label.font_weight),
            (13.0, 500)
        );
        assert_eq!(layout.video_corner_radius, 12.0);
    }

    #[test]
    fn contain_fit_never_crops() {
        let layout = CompositeLayout::embedded();
        let scale = 352.0 / 1448.0;
        assert_rect(
            layout.video_rect(CameraId::Front, Some((1448.0, 938.0))),
            (
                464.0,
                12.0 + (264.0 - 938.0 * scale) / 2.0,
                352.0,
                938.0 * scale,
            ),
        );
        assert_rect(
            layout.video_rect(CameraId::Back, Some((1280.0, 960.0))),
            (480.0, 456.0, 320.0, 240.0),
        );
        assert_rect(
            layout.video_rect(CameraId::Back, None),
            (480.0, 456.0, 320.0, 240.0),
        );
        let destination = Rect {
            x: 400.0,
            y: 12.0,
            width: 480.0,
            height: 216.0,
        };
        assert_rect(
            contain_rect(320.0, 180.0, destination),
            (448.0, 12.0, 384.0, 216.0),
        );
        let square = Rect {
            x: 0.0,
            y: 0.0,
            width: 300.0,
            height: 300.0,
        };
        assert_rect(
            contain_rect(1920.0, 1080.0, square),
            (0.0, 65.625, 300.0, 168.75),
        );
        assert_rect(contain_rect(0.0, 1080.0, square), (0.0, 0.0, 300.0, 300.0));
        assert_rect(
            layout.vehicle_rect(360.0, 720.0),
            (598.0, 285.0, 84.0, 168.0),
        );
    }

    #[test]
    fn label_pills_sit_at_the_bottom_left_of_the_video() {
        let layout = CompositeLayout::embedded();
        let back = layout.video_rect(CameraId::Back, Some((1280.0, 960.0)));
        let pill = layout.label_pill(back, 26.4);
        assert_rect(pill.rect, (490.0, 662.0, 47.0, 24.0));
        assert_eq!(pill.text_origin, (500.0, 679.0));
        let front = layout.video_rect(CameraId::Front, Some((1448.0, 938.0)));
        let pill = layout.label_pill(front, 52.0);
        assert!((pill.rect.y + pill.rect.height + 10.0 - (front.y + front.height)).abs() < 1e-9);
    }

    #[test]
    fn parsing_is_strict() {
        let theme = Theme::embedded();
        let mutate = |edit: &dyn Fn(&mut serde_json::Value)| {
            let mut value: serde_json::Value =
                serde_json::from_str(COMPOSITE_LAYOUT_JSON).expect("json");
            edit(&mut value);
            CompositeLayout::parse(&value.to_string(), theme)
        };
        assert!(mutate(&|value| value["extra"] = 1.into()).is_err());
        assert!(mutate(&|value| value["background"] = "unknown".into()).is_err());
        assert!(mutate(&|value| value["labels"]["back"] = "後ろ".into()).is_err());
        assert!(
            mutate(&|value| {
                value["slots"]
                    .as_object_mut()
                    .expect("slots")
                    .remove("front");
            })
            .is_err()
        );
        assert!(
            mutate(&|value| value["slots"]["cabin"] =
                serde_json::json!({"x": 0, "y": 0, "width": 1, "height": 1}))
            .is_err()
        );
        assert!(mutate(&|_| {}).is_ok());
    }

    fn timeline(segments: &[(&str, &[(CameraId, f64)])]) -> EventTimeline {
        let segments = segments
            .iter()
            .map(|(timestamp, durations)| SegmentMedia {
                timestamp: parse_filename_local_timestamp(timestamp).expect("timestamp"),
                durations: CameraMap::from_fn(|camera| {
                    durations
                        .iter()
                        .find(|(id, _)| *id == camera)
                        .map(|(_, duration)| *duration)
                }),
            })
            .collect::<Vec<_>>();
        EventTimeline::new(&segments).expect("timeline")
    }

    #[test]
    fn the_plan_follows_the_timeline() {
        let all = CameraId::ALL.map(|camera| (camera, 60.0));
        let plan = ExportPlan::new(
            &timeline(&[
                ("2026-08-20_20-10-00", &all),
                ("2026-08-20_20-11-00", &[(CameraId::Front, 20.0)]),
            ]),
            30,
        )
        .expect("plan");
        assert_eq!(plan.duration, 80.0);
        assert_eq!(plan.frame_count, 2_400);
        assert_eq!(plan.segments.len(), 2);
        assert!(plan.segments[0].missing.is_empty());
        assert_eq!(plan.segments[1].present, vec![CameraId::Front]);
        assert_eq!(plan.segments[1].missing.len(), 5);
        assert_eq!(plan.camera_gaps(CameraId::Back), &[(60.0, 80.0)]);
        assert!(plan.camera_gaps(CameraId::Front).is_empty());
        assert_eq!(plan.missing_at(10.0), Vec::<CameraId>::new());
        assert_eq!(plan.missing_at(70.0).len(), 5);
    }

    #[test]
    fn short_clips_and_real_gaps_are_drawn_as_missing() {
        let plan = ExportPlan::new(
            &timeline(&[
                ("2026-08-20_20-10-00", &[(CameraId::Front, 2.0)]),
                ("2026-08-20_20-11-30", &[(CameraId::Front, 2.0)]),
            ]),
            30,
        )
        .expect("plan");
        assert_eq!(plan.duration, 92.0);
        assert_eq!(plan.camera_gaps(CameraId::Front), &[(2.0, 90.0)]);
        assert_eq!(plan.frame_count, 2_760);
        let fractional = ExportPlan::new(
            &timeline(&[("2026-08-20_20-10-00", &[(CameraId::Back, 59.97)])]),
            30,
        )
        .expect("plan");
        assert_eq!(fractional.frame_count, 1_800);
    }

    #[test]
    fn an_event_without_footage_cannot_be_exported() {
        let empty = timeline(&[("2026-08-20_20-10-00", &[])]);
        assert_eq!(
            ExportPlan::new(&empty, 30),
            Err(ExportPlanError::NoPlayableMedia)
        );
    }

    #[test]
    fn progress_maps_to_display_values() {
        let all = CameraId::ALL.map(|camera| (camera, 60.0));
        let plan = ExportPlan::new(
            &timeline(&[("2026-08-20_20-10-00", &all), ("2026-08-20_20-11-00", &all)]),
            30,
        )
        .expect("plan");
        let half = plan.progress(0.5);
        assert_eq!((half.percent, half.elapsed_seconds), (50, 60.0));
        assert_eq!(half.clock, "01:00 / 02:00");
        assert_eq!(half.label, "書き出し中 50%");
        assert_eq!(plan.progress(f64::NAN).fraction, 0.0);
        assert_eq!(plan.progress(3.0).percent, 100);
    }
}
