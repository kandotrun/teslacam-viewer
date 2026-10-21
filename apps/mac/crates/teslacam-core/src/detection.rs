//! Sentry detection time and the 15 s pre-roll.
//!
//! [`detection_seconds`], [`sentry_initial_elapsed_seconds`] and [`sentry_playback_start_label`] are
//! the web's rules on the nominal 60 s axis, pinned by `event-detection.json`. The Mac app uses
//! [`sentry_detection`] and [`initial_position`] on its wall-clock axis instead, with two fixes from
//! the spec header: detection is shown for Sentry events only, and there is no pre-roll (and no
//! pre-roll label) when the detection is earlier than 15 s into the event.

use crate::camera::ClipCategory;
use crate::copy;
use crate::event_timeline::EventTimeline;
use crate::library::Event;
use crate::timestamp::{
    LocalTimestamp, format_filename_timestamp_at_offset, parse_filename_local_timestamp,
    parse_metadata_local_timestamp,
};

/// Seconds of footage shown before a Sentry detection.
pub const PRE_ROLL_SECONDS: i64 = 15;

/// The detection on the web's nominal axis: `i·60 + offset` for the first segment whose 60 s window
/// contains it (the final segment's window includes its end). Milliseconds are dropped. Any category.
pub fn detection_seconds(event: &Event) -> Option<i64> {
    let detected = parse_metadata_local_timestamp(
        event
            .metadata
            .as_ref()
            .map_or("", |metadata| &metadata.timestamp),
    )?;
    let final_index = event.segments.len().checked_sub(1)?;
    event
        .segments
        .iter()
        .enumerate()
        .find_map(|(index, segment)| {
            let start = parse_filename_local_timestamp(&segment.timestamp)?;
            let offset = detected.ordinal_seconds() - start.ordinal_seconds();
            let in_range = if index == final_index {
                (0..=60).contains(&offset)
            } else {
                (0..60).contains(&offset)
            };
            in_range.then_some(index as i64 * 60 + offset)
        })
}

/// Where the web starts a Sentry event: 15 s before the detection, at least 0; 0 for other categories.
pub fn sentry_initial_elapsed_seconds(event: &Event) -> i64 {
    if event.category != ClipCategory::Sentry {
        return 0;
    }
    detection_seconds(event).map_or(0, |detected| (detected - PRE_ROLL_SECONDS).max(0))
}

/// The web's `再生開始` value: the wall-clock moment of [`sentry_initial_elapsed_seconds`].
pub fn sentry_playback_start_label(event: &Event) -> Option<String> {
    if event.category != ClipCategory::Sentry {
        return None;
    }
    let elapsed = sentry_initial_elapsed_seconds(event);
    let segment_index = elapsed.div_euclid(60);
    let segment = event.segments.get(usize::try_from(segment_index).ok()?)?;
    format_filename_timestamp_at_offset(&segment.timestamp, (elapsed - segment_index * 60) as f64)
}

/// A Sentry detection on the Mac's wall-clock axis.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SentryDetection {
    /// The detection moment from `event.json` (milliseconds ignored).
    pub moment: LocalTimestamp,
    /// Event seconds of the detection.
    pub seconds: f64,
    /// Where playback starts, 15 s before the detection; `None` when the detection is earlier than
    /// 15 s into the event (playback then starts at 0 and no pre-roll is announced).
    pub pre_roll_start: Option<f64>,
}

impl SentryDetection {
    /// The details row `再生開始`: `12:01:05（検知の15秒前）`, or the event start with
    /// `（記録の最初から）` when there was no pre-roll.
    pub fn playback_start_text(&self, timeline: &EventTimeline) -> Option<String> {
        let start = self.pre_roll_start.unwrap_or(0.0);
        let moment = timeline.wall_clock_at(start)?;
        Some(copy::playback_start_text(
            &moment.time_label(),
            self.pre_roll_start.is_some(),
        ))
    }
}

/// The detection of a Sentry event that lies inside its timeline; `None` for other categories, a
/// missing or malformed `event.json` timestamp, or a detection outside the event.
pub fn sentry_detection(event: &Event, timeline: &EventTimeline) -> Option<SentryDetection> {
    if event.category != ClipCategory::Sentry {
        return None;
    }
    let moment = parse_metadata_local_timestamp(&event.metadata.as_ref()?.timestamp)?;
    let seconds = timeline.seconds_at_wall_clock(moment.ordinal_seconds())?;
    let pre_roll = PRE_ROLL_SECONDS as f64;
    Some(SentryDetection {
        moment,
        seconds,
        pre_roll_start: (seconds >= pre_roll).then_some(seconds - pre_roll),
    })
}

/// Where a new theater session starts: the pre-roll start of a Sentry detection, else 0.
pub fn initial_position(event: &Event, timeline: &EventTimeline) -> f64 {
    sentry_detection(event, timeline)
        .and_then(|detection| detection.pre_roll_start)
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::contract::{event_input, option_json, run_fixture};

    #[test]
    fn matches_the_event_detection_fixture() {
        run_fixture("event-detection", |case| {
            let event = event_input(case.input("event"));
            Some(match case.function.as_str() {
                "detectionSeconds" => option_json(detection_seconds(&event)),
                "sentryInitialElapsedSeconds" => {
                    Value::from(sentry_initial_elapsed_seconds(&event))
                }
                "sentryPlaybackStartLabel" => option_json(sentry_playback_start_label(&event)),
                _ => return None,
            })
        });
    }

    fn event(category: &str, segments: &[&str], detection: &str) -> Event {
        event_input(&json!({
            "id": "event",
            "category": category,
            "timestamp": segments[0],
            "segments": segments.iter().map(|timestamp| json!({"timestamp": timestamp, "cameras": ["front"]})).collect::<Vec<_>>(),
            "metadata": {"timestamp": detection},
        }))
    }

    fn mac_detection(event: &Event) -> (Option<SentryDetection>, f64, EventTimeline) {
        let timeline = EventTimeline::assuming_full_windows(event).expect("timeline");
        (
            sentry_detection(event, &timeline),
            initial_position(event, &timeline),
            timeline,
        )
    }

    #[test]
    fn pre_roll_starts_fifteen_seconds_before_the_detection() {
        let sentry = event("sentry", &["2026-08-20_20-35-00"], "2026-08-20T20:35:30");
        let (detection, initial, timeline) = mac_detection(&sentry);
        let detection = detection.expect("detection");
        assert_eq!(detection.seconds, 30.0);
        assert_eq!(detection.pre_roll_start, Some(15.0));
        assert_eq!(initial, 15.0);
        assert_eq!(
            detection.playback_start_text(&timeline).as_deref(),
            Some("20:35:15（検知の15秒前）")
        );
    }

    #[test]
    fn no_pre_roll_when_the_detection_is_earlier_than_fifteen_seconds() {
        let early = event("sentry", &["2026-08-24_12-00-00"], "2026-08-24T12:00:14");
        let (detection, initial, timeline) = mac_detection(&early);
        let detection = detection.expect("detection");
        assert_eq!(detection.pre_roll_start, None);
        assert_eq!(
            detection.playback_start_text(&timeline).as_deref(),
            Some("12:00:00（記録の最初から）")
        );
        assert_eq!(initial, 0.0);
        let boundary = event("sentry", &["2026-08-24_12-00-00"], "2026-08-24T12:00:15");
        assert_eq!(
            mac_detection(&boundary)
                .0
                .and_then(|detection| detection.pre_roll_start),
            Some(0.0)
        );
    }

    #[test]
    fn detection_is_shown_for_sentry_events_only() {
        let saved = event("saved", &["2026-08-24_12-00-00"], "2026-08-24T12:00:30");
        let (detection, initial, _) = mac_detection(&saved);
        assert_eq!(detection, None);
        assert_eq!(initial, 0.0);
        assert_eq!(detection_seconds(&saved), Some(30));
    }

    #[test]
    fn wall_clock_detection_inside_a_gap() {
        let gap = event(
            "sentry",
            &["2026-08-24_12-00-00", "2026-08-24_12-01-11"],
            "2026-08-24T12:01:05",
        );
        let (detection, initial, _) = mac_detection(&gap);
        assert_eq!(detection.map(|detection| detection.seconds), Some(65.0));
        assert_eq!(initial, 50.0);
        assert_eq!(detection_seconds(&gap), None);
    }

    #[test]
    fn detections_outside_the_event_or_malformed_are_ignored() {
        let late = event("sentry", &["2026-08-24_12-00-00"], "2026-08-24T12:01:01");
        assert_eq!(mac_detection(&late).0, None);
        let malformed = event(
            "sentry",
            &["2026-08-24_12-00-00"],
            "2026-08-24T12:00:30+09:00",
        );
        assert_eq!(mac_detection(&malformed).0, None);
        let precise = event(
            "sentry",
            &["2026-08-24_12-00-00"],
            "2026-08-24T12:00:05.900Z",
        );
        assert_eq!(
            mac_detection(&precise).0.map(|detection| detection.seconds),
            Some(5.0)
        );
    }
}
