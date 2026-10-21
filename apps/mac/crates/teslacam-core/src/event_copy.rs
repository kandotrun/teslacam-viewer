//! What the library rows, the event pill and the details popover say about an event.
//!
//! Pure: views render these strings verbatim. Durations and detection use the Mac's wall-clock
//! [`EventTimeline`]; detection is shown for Sentry events only (`detection::sentry_detection`).

use crate::camera::ClipCategory;
use crate::copy::{self, DetectionParts};
use crate::detection::sentry_detection;
use crate::event_timeline::EventTimeline;
use crate::filters::event_time;
use crate::library::Event;
use crate::summary::{self, camera_availability};

/// One row of the event list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRowCopy {
    /// Accessible name, e.g. `2026-08-20 20:45:00`.
    pub label: String,
    /// `20:45`
    pub start_clock: String,
    /// The chip.
    pub category: ClipCategory,
    /// e.g. `2分`
    pub duration: String,
    /// e.g. `6カメラ`
    pub cameras: String,
    /// Sentry only, e.g. `20:35:30に検知`.
    pub detection: Option<String>,
    /// The warning line, e.g. `右リピーターの記録がありません`.
    pub missing: Option<String>,
}

/// The event pill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventPillCopy {
    /// The chip (full category name).
    pub category: ClipCategory,
    /// `20:35` (followed by [`copy::EVENT_START_SUFFIX`]).
    pub start_clock: String,
    /// City and street from `event.json`.
    pub location: Option<String>,
    /// Sentry detection in words and time.
    pub detection: Option<DetectionParts>,
}

/// One row of the details popover.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetailRow {
    /// e.g. `長さ`
    pub label: &'static str,
    /// e.g. `1分`
    pub value: String,
    /// A fault under the value (missing cameras).
    pub warning: Option<String>,
}

/// The details popover: rows shown only when their data exists, and the raw `reason` for the closed
/// `技術情報` disclosure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventDetails {
    /// Rows in display order.
    pub rows: Vec<DetailRow>,
    /// `event.json`'s `reason`.
    pub technical: Option<String>,
}

/// The duration text on the wall-clock timeline, rounded to whole seconds.
pub fn duration_text(timeline: &EventTimeline) -> String {
    copy::format_duration(timeline.duration().max(0.0).round() as u64)
}

fn full_window_duration(event: &Event) -> String {
    EventTimeline::assuming_full_windows(event).map_or_else(
        |_| summary::event_duration_label(event),
        |timeline| duration_text(&timeline),
    )
}

/// The row of `event` in the list (its timeline before clip lengths are known).
pub fn event_row(event: &Event) -> EventRowCopy {
    let availability = camera_availability(event);
    let detection = EventTimeline::assuming_full_windows(event)
        .ok()
        .and_then(|timeline| sentry_detection(event, &timeline))
        .map(|detection| copy::detection_time_text(&detection.moment.time_label()));
    EventRowCopy {
        label: event_time(&event.timestamp),
        start_clock: copy::start_clock(&event.timestamp),
        category: event.category,
        duration: full_window_duration(event),
        cameras: copy::camera_count_text(availability.available),
        detection,
        missing: copy::missing_cameras_text(&availability.missing),
    }
}

fn reason(event: &Event) -> Option<&str> {
    event.metadata.as_ref()?.reason.as_deref()
}

fn location(event: &Event) -> Option<String> {
    let metadata = event.metadata.as_ref()?;
    copy::location_text(metadata.city.as_deref(), metadata.street.as_deref())
}

/// The event pill of `event` on `timeline`.
pub fn event_pill(event: &Event, timeline: &EventTimeline) -> EventPillCopy {
    EventPillCopy {
        category: event.category,
        start_clock: copy::start_clock(&event.timestamp),
        location: location(event),
        detection: sentry_detection(event, timeline)
            .map(|detection| copy::detection_parts(reason(event), &detection.moment.time_label())),
    }
}

/// The details popover of `event` on `timeline`.
pub fn event_details(event: &Event, timeline: &EventTimeline) -> EventDetails {
    let availability = camera_availability(event);
    let detection = sentry_detection(event, timeline);
    let metadata = event.metadata.as_ref();
    let row = |label, value: String| DetailRow {
        label,
        value,
        warning: None,
    };
    let mut rows = Vec::new();
    if let Some(location) = location(event) {
        rows.push(row(copy::DETAILS_LOCATION, location));
    }
    if let Some(coordinates) = metadata.and_then(|metadata| {
        copy::coordinates_text(metadata.est_lat.as_deref(), metadata.est_lon.as_deref())
    }) {
        rows.push(row(copy::DETAILS_COORDINATES, coordinates));
    }
    if let Some(detection) = &detection {
        rows.push(row(
            copy::DETAILS_DETECTION,
            copy::detection_summary(reason(event), &detection.moment.time_label()),
        ));
        if let Some(start) = detection.playback_start_text(timeline) {
            rows.push(row(copy::DETAILS_PLAYBACK_START, start));
        }
    }
    rows.push(DetailRow {
        label: copy::DETAILS_CAMERAS,
        value: copy::camera_count_text(availability.available),
        warning: copy::missing_cameras_text(&availability.missing),
    });
    rows.push(row(copy::DETAILS_DURATION, duration_text(timeline)));
    rows.push(row(
        copy::DETAILS_SEGMENTS,
        copy::segment_count_label(event.segments.len()),
    ));
    rows.push(row(
        copy::DETAILS_RECORDING_START,
        event_time(&event.timestamp),
    ));
    EventDetails {
        rows,
        technical: reason(event).map(str::to_owned),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::contract::event_input;

    fn sentry(cameras: Value, metadata: Value) -> Event {
        event_input(&json!({
            "id": "sentry-2026-08-20_20-35-00-1", "category": "sentry",
            "timestamp": "2026-08-20_20-35-00",
            "segments": [{"timestamp": "2026-08-20_20-35-00", "cameras": cameras}],
            "metadata": metadata,
        }))
    }

    fn all_cameras() -> Value {
        json!([
            "front",
            "back",
            "left_pillar",
            "left_repeater",
            "right_pillar",
            "right_repeater"
        ])
    }

    fn timeline(event: &Event) -> EventTimeline {
        EventTimeline::assuming_full_windows(event).expect("timeline")
    }

    #[test]
    fn rows_state_duration_cameras_detection_and_faults() {
        let event = sentry(
            json!([
                "front",
                "back",
                "left_pillar",
                "left_repeater",
                "right_pillar"
            ]),
            json!({"timestamp": "2026-08-20T20:35:30", "reason": "sentry_aware_object_detection"}),
        );
        assert_eq!(
            event_row(&event),
            EventRowCopy {
                label: "2026-08-20 20:35:00".into(),
                start_clock: "20:35".into(),
                category: ClipCategory::Sentry,
                duration: "1分".into(),
                cameras: "5カメラ".into(),
                detection: Some("20:35:30に検知".into()),
                missing: Some("右リピーターの記録がありません".into()),
            }
        );
    }

    #[test]
    fn saved_events_never_show_a_detection() {
        let event = event_input(&json!({
            "id": "saved-1", "category": "saved", "timestamp": "2026-08-20_20-40-00",
            "segments": [
                {"timestamp": "2026-08-20_20-40-00", "cameras": all_cameras()},
                {"timestamp": "2026-08-20_20-41-00", "cameras": all_cameras()},
            ],
            "metadata": {"timestamp": "2026-08-20T20:40:30", "reason": "user_interaction_honk",
                         "city": "渋谷区", "street": "神南一丁目"},
        }));
        let row = event_row(&event);
        assert_eq!(row.detection, None);
        assert_eq!(row.missing, None);
        assert_eq!(
            (row.duration.as_str(), row.cameras.as_str()),
            ("2分", "6カメラ")
        );
        let pill = event_pill(&event, &timeline(&event));
        assert_eq!(pill.detection, None);
        assert_eq!(pill.location.as_deref(), Some("渋谷区 神南一丁目"));
    }

    #[test]
    fn the_pill_names_the_reason_and_time() {
        let event = sentry(
            all_cameras(),
            json!({"timestamp": "2026-08-20T20:35:30", "reason": "sentry_aware_object_detection",
                   "city": "SYNTHETIC_CITY"}),
        );
        let pill = event_pill(&event, &timeline(&event));
        assert_eq!(pill.start_clock, "20:35");
        assert_eq!(pill.location.as_deref(), Some("SYNTHETIC_CITY"));
        let detection = pill.detection.expect("detection");
        assert_eq!(
            (
                detection.lead.as_str(),
                detection.time.as_str(),
                detection.tail
            ),
            ("物体の接近を", "20:35:30", "に検知")
        );
    }

    #[test]
    fn details_show_only_rows_with_data() {
        let event = sentry(
            json!([
                "front",
                "back",
                "left_pillar",
                "left_repeater",
                "right_pillar"
            ]),
            json!({"timestamp": "2026-08-20T20:35:30", "reason": "sentry_aware_object_detection",
                   "city": "渋谷区", "street": "", "est_lat": "35.6595", "est_lon": "139.7005"}),
        );
        let details = event_details(&event, &timeline(&event));
        let labels = details.rows.iter().map(|row| row.label).collect::<Vec<_>>();
        assert_eq!(
            labels,
            [
                "場所（Teslaの推定値）",
                "推定座標",
                "検知",
                "再生開始",
                "カメラ",
                "長さ",
                "セグメント",
                "録画開始"
            ]
        );
        let values = details
            .rows
            .iter()
            .map(|row| row.value.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            values,
            [
                "渋谷区",
                "35.6595, 139.7005",
                "物体の接近を20:35:30に検知",
                "20:35:15（検知の15秒前）",
                "5カメラ",
                "1分",
                "1セグメント",
                "2026-08-20 20:35:00"
            ]
        );
        assert_eq!(
            details.rows[4].warning.as_deref(),
            Some("右リピーターの記録がありません")
        );
        assert_eq!(
            details.technical.as_deref(),
            Some("sentry_aware_object_detection")
        );
    }

    #[test]
    fn details_without_metadata_keep_the_basics() {
        let event = event_input(&json!({
            "id": "recent-1", "category": "recent", "timestamp": "2026-08-20_20-45-00",
            "segments": [{"timestamp": "2026-08-20_20-45-00", "cameras": all_cameras()}],
        }));
        let details = event_details(&event, &timeline(&event));
        assert_eq!(
            details.rows.iter().map(|row| row.label).collect::<Vec<_>>(),
            ["カメラ", "長さ", "セグメント", "録画開始"]
        );
        assert_eq!(details.technical, None);
    }
}
