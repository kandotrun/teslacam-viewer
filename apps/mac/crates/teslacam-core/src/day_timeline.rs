//! The day rail: markers per event on a 0–24 h track, recorded days, and clusters of markers whose
//! hit areas overlap at the measured track width.

use std::cmp::Ordering;

use crate::camera::ClipCategory;
use crate::library::Event;
use crate::timestamp::{LocalTimestamp, parse_filename_local_timestamp};

/// Seconds of day at the right end of the track (23:59:59 → 100 %).
pub const FINAL_SECOND_OF_DAY: f64 = 86_399.0;
/// Width of a marker's hit area in points; closer markers form a cluster.
pub const MARKER_HIT_WIDTH: f64 = 44.0;

/// One event on the day rail.
#[derive(Debug, Clone, PartialEq)]
pub struct DayTimelineMarker {
    /// The event id.
    pub event_id: String,
    /// The event category (marker colour).
    pub category: ClipCategory,
    /// `YYYY-MM-DD`.
    pub date_key: String,
    /// `hh:mm:ss`.
    pub time_label: String,
    /// Seconds since local midnight.
    pub seconds_of_day: u32,
    /// `seconds_of_day / 86 399 × 100`.
    pub position_percent: f64,
}

/// A day that has recordings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineDay {
    /// `YYYY-MM-DD`.
    pub date_key: String,
    /// The day's events in marker order (time, then Sentry, Saved, Recent, then id).
    pub event_ids: Vec<String>,
}

impl TimelineDay {
    /// The newest event of the day (the last marker): what day navigation selects.
    pub fn newest_event_id(&self) -> Option<&str> {
        self.event_ids.last().map(String::as_str)
    }
}

/// The recorded days around the current one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DayAdjacency<'a> {
    /// The latest recorded day before the current one.
    pub previous: Option<&'a TimelineDay>,
    /// The earliest recorded day after the current one.
    pub next: Option<&'a TimelineDay>,
}

/// Markers whose hit areas overlap, shown as one pill.
#[derive(Debug, Clone, PartialEq)]
pub struct DayTimelineCluster {
    /// The members in marker order (at least one).
    pub markers: Vec<DayTimelineMarker>,
}

impl DayTimelineCluster {
    fn first(&self) -> &DayTimelineMarker {
        &self.markers[0]
    }

    fn last(&self) -> &DayTimelineMarker {
        &self.markers[self.markers.len() - 1]
    }

    /// The first member's event id.
    pub fn id(&self) -> &str {
        &self.first().event_id
    }

    /// Number of members.
    pub fn count(&self) -> usize {
        self.markers.len()
    }

    /// First member's `hh:mm:ss`.
    pub fn start_time_label(&self) -> &str {
        &self.first().time_label
    }

    /// Last member's `hh:mm:ss`.
    pub fn end_time_label(&self) -> &str {
        &self.last().time_label
    }

    /// First member's seconds of day.
    pub fn start_seconds_of_day(&self) -> u32 {
        self.first().seconds_of_day
    }

    /// Last member's seconds of day.
    pub fn end_seconds_of_day(&self) -> u32 {
        self.last().seconds_of_day
    }

    /// Span in seconds.
    pub fn duration_seconds(&self) -> u32 {
        self.end_seconds_of_day() - self.start_seconds_of_day()
    }

    /// First member's position.
    pub fn start_position_percent(&self) -> f64 {
        self.first().position_percent
    }

    /// Last member's position.
    pub fn end_position_percent(&self) -> f64 {
        self.last().position_percent
    }

    /// Where the pill is drawn: the midpoint of the span.
    pub fn center_percent(&self) -> f64 {
        (self.start_position_percent() + self.end_position_percent()) / 2.0
    }

    /// Whether `event_id` is a member.
    pub fn contains(&self, event_id: &str) -> bool {
        self.markers
            .iter()
            .any(|marker| marker.event_id == event_id)
    }
}

/// The timestamp that places an event on the rail (a Recent session uses its first segment).
pub fn timeline_timestamp(event: &Event) -> Option<LocalTimestamp> {
    let timestamp = match event.category {
        ClipCategory::Recent => event
            .segments
            .first()
            .map_or(event.timestamp.as_str(), |segment| {
                segment.timestamp.as_str()
            }),
        _ => event.timestamp.as_str(),
    };
    parse_filename_local_timestamp(timestamp)
}

fn category_rank(category: ClipCategory) -> u8 {
    match category {
        ClipCategory::Sentry => 0,
        ClipCategory::Saved => 1,
        ClipCategory::Recent => 2,
    }
}

fn compare_markers(left: &DayTimelineMarker, right: &DayTimelineMarker) -> Ordering {
    left.seconds_of_day
        .cmp(&right.seconds_of_day)
        .then_with(|| category_rank(left.category).cmp(&category_rank(right.category)))
        .then_with(|| left.event_id.cmp(&right.event_id))
}

fn marker(event: &Event) -> Option<DayTimelineMarker> {
    let timestamp = timeline_timestamp(event)?;
    Some(DayTimelineMarker {
        event_id: event.id.clone(),
        category: event.category,
        date_key: timestamp.date_key(),
        time_label: timestamp.time_label(),
        seconds_of_day: timestamp.seconds_of_day(),
        position_percent: f64::from(timestamp.seconds_of_day()) / FINAL_SECOND_OF_DAY * 100.0,
    })
}

/// A valid `YYYY-MM-DD` normalized through the calendar, else `None`.
fn parsed_date_key(date_key: Option<&str>) -> Option<String> {
    parse_filename_local_timestamp(&format!("{}_00-00-00", date_key?))
        .map(|timestamp| timestamp.date_key())
}

/// The day the rail shows: the selected event's day, else a valid date filter, else the first event
/// that parses.
pub fn select_timeline_date(
    events: &[&Event],
    selected: Option<&Event>,
    filter_date_key: Option<&str>,
) -> Option<String> {
    selected
        .and_then(timeline_timestamp)
        .map(|timestamp| timestamp.date_key())
        .or_else(|| parsed_date_key(filter_date_key))
        .or_else(|| {
            events
                .iter()
                .find_map(|event| timeline_timestamp(event))
                .map(|timestamp| timestamp.date_key())
        })
}

/// The markers of `date_key`, in marker order; empty for a missing or invalid date.
pub fn build_day_timeline_markers(
    events: &[&Event],
    date_key: Option<&str>,
) -> Vec<DayTimelineMarker> {
    let Some(date_key) = parsed_date_key(date_key) else {
        return Vec::new();
    };
    let mut markers = events
        .iter()
        .filter_map(|event| marker(event))
        .filter(|marker| marker.date_key == date_key)
        .collect::<Vec<_>>();
    markers.sort_by(compare_markers);
    markers
}

/// Every recorded day in ascending order.
pub fn build_timeline_days(events: &[&Event]) -> Vec<TimelineDay> {
    let mut markers = events
        .iter()
        .filter_map(|event| marker(event))
        .collect::<Vec<_>>();
    markers.sort_by(|left, right| {
        left.date_key
            .cmp(&right.date_key)
            .then_with(|| compare_markers(left, right))
    });
    let mut days: Vec<TimelineDay> = Vec::new();
    for marker in markers {
        match days.last_mut() {
            Some(day) if day.date_key == marker.date_key => day.event_ids.push(marker.event_id),
            _ => days.push(TimelineDay {
                date_key: marker.date_key,
                event_ids: vec![marker.event_id],
            }),
        }
    }
    days
}

/// The recorded days before and after `current_date_key` (both `None` for an invalid date).
pub fn adjacent_timeline_days<'a>(
    days: &'a [TimelineDay],
    current_date_key: Option<&str>,
) -> DayAdjacency<'a> {
    let Some(current) = parsed_date_key(current_date_key) else {
        return DayAdjacency {
            previous: None,
            next: None,
        };
    };
    DayAdjacency {
        previous: days
            .iter()
            .filter(|day| day.date_key < current)
            .max_by(|left, right| left.date_key.cmp(&right.date_key)),
        next: days
            .iter()
            .filter(|day| day.date_key > current)
            .min_by(|left, right| left.date_key.cmp(&right.date_key)),
    }
}

/// The markers of `date_key` grouped into clusters for a track `track_width` points wide.
///
/// A marker joins the current cluster when it is closer than [`MARKER_HIT_WIDTH`] to the previous
/// marker (chained, not measured from the cluster start). A width that is not finite and positive
/// never clusters. Pass the measured rail width; the web uses a fixed 640.
pub fn build_day_timeline_clusters(
    events: &[&Event],
    date_key: Option<&str>,
    track_width: f64,
) -> Vec<DayTimelineCluster> {
    let clusters_enabled = track_width.is_finite() && track_width > 0.0;
    let mut clusters: Vec<DayTimelineCluster> = Vec::new();
    for marker in build_day_timeline_markers(events, date_key) {
        match clusters.last_mut() {
            Some(cluster)
                if clusters_enabled
                    && (marker.position_percent - cluster.last().position_percent) / 100.0
                        * track_width
                        < MARKER_HIT_WIDTH =>
            {
                cluster.markers.push(marker)
            }
            _ => clusters.push(DayTimelineCluster {
                markers: vec![marker],
            }),
        }
    }
    clusters
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::contract::{events_input, number_input, optional_string, run_fixture};

    fn marker_json(marker: &DayTimelineMarker) -> Value {
        json!({
            "eventId": marker.event_id,
            "category": marker.category.id(),
            "dateKey": marker.date_key,
            "timeLabel": marker.time_label,
            "secondsOfDay": marker.seconds_of_day,
            "positionPercent": marker.position_percent,
        })
    }

    fn day_json(day: Option<&TimelineDay>) -> Value {
        day.map_or(
            Value::Null,
            |day| json!({"dateKey": day.date_key, "eventIds": day.event_ids}),
        )
    }

    fn cluster_json(cluster: &DayTimelineCluster) -> Value {
        json!({
            "id": cluster.id(),
            "count": cluster.count(),
            "eventIds": cluster.markers.iter().map(|marker| marker.event_id.clone()).collect::<Vec<_>>(),
            "startTimeLabel": cluster.start_time_label(),
            "endTimeLabel": cluster.end_time_label(),
            "startSecondsOfDay": cluster.start_seconds_of_day(),
            "endSecondsOfDay": cluster.end_seconds_of_day(),
            "durationSeconds": cluster.duration_seconds(),
            "startPositionPercent": cluster.start_position_percent(),
            "endPositionPercent": cluster.end_position_percent(),
        })
    }

    fn days_input(value: &Value) -> Vec<TimelineDay> {
        value
            .as_array()
            .expect("days")
            .iter()
            .map(|day| TimelineDay {
                date_key: day["dateKey"].as_str().expect("dateKey").to_owned(),
                event_ids: day["eventIds"]
                    .as_array()
                    .expect("eventIds")
                    .iter()
                    .map(|id| id.as_str().expect("id").to_owned())
                    .collect(),
            })
            .collect()
    }

    #[test]
    fn matches_the_day_timeline_fixture() {
        run_fixture("day-timeline", |case| {
            let events = case
                .input
                .get("events")
                .map(events_input)
                .unwrap_or_default();
            let refs = events.iter().collect::<Vec<_>>();
            let date_key = || optional_string(case.input("dateKey"));
            Some(match case.function.as_str() {
                "selectTimelineDate" => {
                    let selected_id = optional_string(case.input("selectedEventId"));
                    let selected = events
                        .iter()
                        .find(|event| Some(&event.id) == selected_id.as_ref());
                    let filter = optional_string(case.input("filterDateKey"));
                    select_timeline_date(&refs, selected, filter.as_deref())
                        .map_or(Value::Null, Value::from)
                }
                "buildDayTimelineMarkers" => {
                    build_day_timeline_markers(&refs, date_key().as_deref())
                        .iter()
                        .map(marker_json)
                        .collect::<Vec<_>>()
                        .into()
                }
                "buildTimelineDays" => build_timeline_days(&refs)
                    .iter()
                    .map(|day| day_json(Some(day)))
                    .collect::<Vec<_>>()
                    .into(),
                "adjacentTimelineDays" => {
                    let days = days_input(case.input("days"));
                    let current = optional_string(case.input("currentDateKey"));
                    let adjacency = adjacent_timeline_days(&days, current.as_deref());
                    json!({"previous": day_json(adjacency.previous), "next": day_json(adjacency.next)})
                }
                "buildDayTimelineClusters" => {
                    let width = number_input(case.input("trackWidthPixels"));
                    build_day_timeline_clusters(&refs, date_key().as_deref(), width)
                        .iter()
                        .map(cluster_json)
                        .collect::<Vec<_>>()
                        .into()
                }
                _ => return None,
            })
        });
    }

    #[test]
    fn day_navigation_targets_the_newest_event() {
        let day = TimelineDay {
            date_key: "2026-08-23".into(),
            event_ids: vec!["previous-morning".into(), "previous-evening".into()],
        };
        assert_eq!(day.newest_event_id(), Some("previous-evening"));
        let empty = TimelineDay {
            date_key: "2026-08-23".into(),
            event_ids: Vec::new(),
        };
        assert_eq!(empty.newest_event_id(), None);
    }

    #[test]
    fn a_narrower_rail_clusters_more() {
        let events = events_input(&json!([
            {"id": "a", "category": "saved", "timestamp": "2026-08-24_20-35-00", "segments": [{"timestamp": "2026-08-24_20-35-00", "cameras": []}]},
            {"id": "b", "category": "saved", "timestamp": "2026-08-24_21-15-00", "segments": [{"timestamp": "2026-08-24_21-15-00", "cameras": []}]},
        ]));
        let refs = events.iter().collect::<Vec<_>>();
        assert_eq!(
            build_day_timeline_clusters(&refs, Some("2026-08-24"), 2_000.0).len(),
            2
        );
        let narrow = build_day_timeline_clusters(&refs, Some("2026-08-24"), 340.0);
        assert_eq!(narrow.len(), 1);
        assert!(narrow[0].contains("b"));
        assert!(
            (narrow[0].center_percent()
                - (74_100.0 + 76_500.0) / 2.0 / FINAL_SECOND_OF_DAY * 100.0)
                .abs()
                < 1e-9
        );
    }
}
