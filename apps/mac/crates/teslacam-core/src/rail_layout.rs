//! Horizontal placement of the day rail's interactive items (single markers and cluster pills) so
//! they never overlap and stay inside the track (the web's `presentation/rail-layout.ts`).
//!
//! [`day_rail`] combines it with [`build_day_timeline_clusters`] for the measured track width:
//! markers whose 44 pt hit areas overlap become one cluster pill; a lone marker keeps an invisible
//! 44 pt hit area centred on it. Pills show the time range and the count when every item fits side by
//! side, otherwise only the count.

use crate::day_timeline::{
    DayTimelineCluster, DayTimelineMarker, MARKER_HIT_WIDTH, build_day_timeline_clusters,
    build_day_timeline_markers,
};
use crate::library::Event;

/// Space kept between neighbouring items.
pub const ITEM_GAP: f64 = 2.0;
/// Width of a cluster pill with its time range and count.
pub const FULL_PILL_WIDTH: f64 = 104.0;
/// Width of a cluster pill with only its count.
pub const COMPACT_PILL_WIDTH: f64 = 44.0;

/// An item to place: its desired centre and its width, in points from the track's left edge.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RailItemRequest {
    /// Desired centre.
    pub center: f64,
    /// Item width.
    pub width: f64,
}

/// Left edges for items given in ascending centre order: centred where there is room, pushed apart
/// by [`ITEM_GAP`] where they would overlap, and kept inside `[0, track_width]`.
pub fn layout_rail_items(items: &[RailItemRequest], track_width: f64) -> Vec<f64> {
    let mut lefts = items
        .iter()
        .map(|item| {
            (item.center - item.width / 2.0)
                .max(0.0)
                .min((track_width - item.width).max(0.0))
        })
        .collect::<Vec<_>>();
    for index in 1..items.len() {
        lefts[index] = lefts[index].max(lefts[index - 1] + items[index - 1].width + ITEM_GAP);
    }
    for index in (0..items.len()).rev() {
        let next_left = lefts
            .get(index + 1)
            .copied()
            .unwrap_or(track_width + ITEM_GAP);
        lefts[index] = lefts[index]
            .min(next_left - ITEM_GAP - items[index].width)
            .max(0.0);
    }
    lefts
}

/// Whether every item fits side by side (with [`ITEM_GAP`] between them).
pub fn rail_pills_fit(item_widths: &[f64], track_width: f64) -> bool {
    let gaps = item_widths.len().saturating_sub(1) as f64 * ITEM_GAP;
    item_widths.iter().sum::<f64>() + gaps <= track_width
}

/// One interactive item on the rail.
#[derive(Debug, Clone, PartialEq)]
pub struct RailItem {
    /// The markers it stands for (one for a lone marker).
    pub cluster: DayTimelineCluster,
    /// Left edge in points from the track's left edge.
    pub left: f64,
    /// Width in points.
    pub width: f64,
}

impl RailItem {
    /// A lone marker (an invisible hit area) rather than a cluster pill.
    pub fn is_marker(&self) -> bool {
        self.cluster.count() == 1
    }
}

/// What the rail draws for one day.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DayRail {
    /// Every marker of the day, in marker order.
    pub markers: Vec<DayTimelineMarker>,
    /// The markers drawn as coloured marks: those that are not part of a cluster pill.
    pub loose_markers: Vec<DayTimelineMarker>,
    /// Hit areas and cluster pills, left to right.
    pub items: Vec<RailItem>,
    /// Whether the pills show their time range (otherwise only the count).
    pub full_pills: bool,
}

/// The rail of `date_key` for a track `track_width` points wide (the measured width).
pub fn day_rail(events: &[&Event], date_key: Option<&str>, track_width: f64) -> DayRail {
    let markers = build_day_timeline_markers(events, date_key);
    let clusters = build_day_timeline_clusters(events, date_key, track_width);
    let pill_width = |cluster: &DayTimelineCluster, full: bool| match (cluster.count(), full) {
        (1, _) => MARKER_HIT_WIDTH,
        (_, true) => FULL_PILL_WIDTH,
        (_, false) => COMPACT_PILL_WIDTH,
    };
    let full_pills = rail_pills_fit(
        &clusters
            .iter()
            .map(|cluster| pill_width(cluster, true))
            .collect::<Vec<_>>(),
        track_width,
    );
    let widths = clusters
        .iter()
        .map(|cluster| pill_width(cluster, full_pills))
        .collect::<Vec<_>>();
    let width = track_width.max(0.0);
    let lefts = layout_rail_items(
        &clusters
            .iter()
            .zip(&widths)
            .map(|(cluster, item_width)| RailItemRequest {
                center: cluster.center_percent() / 100.0 * width,
                width: *item_width,
            })
            .collect::<Vec<_>>(),
        width,
    );
    let loose_markers = markers
        .iter()
        .filter(|marker| {
            clusters
                .iter()
                .any(|cluster| cluster.count() == 1 && cluster.contains(&marker.event_id))
        })
        .cloned()
        .collect();
    DayRail {
        markers,
        loose_markers,
        items: clusters
            .into_iter()
            .zip(lefts.into_iter().zip(widths))
            .map(|(cluster, (left, width))| RailItem {
                cluster,
                left,
                width,
            })
            .collect(),
        full_pills,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::contract::event_input;

    fn request(center: f64, width: f64) -> RailItemRequest {
        RailItemRequest { center, width }
    }

    #[test]
    fn centres_items_that_have_room() {
        assert_eq!(
            layout_rail_items(&[request(50.0, 44.0), request(200.0, 44.0)], 300.0),
            [28.0, 178.0]
        );
    }

    #[test]
    fn keeps_items_inside_the_track() {
        assert_eq!(
            layout_rail_items(&[request(5.0, 44.0), request(298.0, 96.0)], 300.0),
            [0.0, 204.0]
        );
    }

    #[test]
    fn pushes_a_pill_away_from_a_neighbour_instead_of_overlapping_it() {
        assert_eq!(
            layout_rail_items(&[request(100.0, 44.0), request(150.0, 96.0)], 300.0),
            [78.0, 124.0]
        );
    }

    #[test]
    fn pulls_items_back_from_the_right_edge_when_they_crowd_it() {
        assert_eq!(
            layout_rail_items(&[request(280.0, 44.0), request(290.0, 44.0)], 300.0),
            [210.0, 256.0]
        );
    }

    #[test]
    fn reports_whether_every_item_fits_side_by_side() {
        assert!(rail_pills_fit(&[96.0, 96.0, 44.0], 300.0));
        assert!(!rail_pills_fit(&[96.0, 96.0, 96.0, 44.0], 300.0));
        assert!(rail_pills_fit(&[], 0.0));
    }

    fn events() -> Vec<Event> {
        [
            ("sentry-a", "sentry", "2026-08-20_20-35-00"),
            ("saved-a", "saved", "2026-08-20_20-40-00"),
            ("recent-a", "recent", "2026-08-20_20-45-00"),
            ("recent-b", "recent", "2026-08-20_06-00-00"),
            ("recent-c", "recent", "2026-08-21_06-00-00"),
        ]
        .into_iter()
        .map(|(id, category, timestamp)| {
            event_input(&json!({
                "id": id, "category": category, "timestamp": timestamp,
                "segments": [{"timestamp": timestamp, "cameras": ["front"]}],
            }))
        })
        .collect()
    }

    #[test]
    fn the_rail_clusters_close_markers_and_keeps_lone_ones_as_marks() {
        let events = events();
        let refs = events.iter().collect::<Vec<_>>();
        let rail = day_rail(&refs, Some("2026-08-20"), 330.0);
        assert_eq!(rail.markers.len(), 4);
        assert_eq!(rail.items.len(), 2);
        assert!(rail.items[0].is_marker());
        assert_eq!(rail.items[0].cluster.id(), "recent-b");
        assert_eq!(rail.items[1].cluster.count(), 3);
        assert!(rail.full_pills);
        assert_eq!(rail.items[1].width, FULL_PILL_WIDTH);
        assert_eq!(
            rail.loose_markers
                .iter()
                .map(|marker| marker.event_id.as_str())
                .collect::<Vec<_>>(),
            ["recent-b"]
        );
        // The pill is centred on its span and kept inside the track.
        let pill = &rail.items[1];
        assert!(pill.left >= 0.0 && pill.left + pill.width <= 330.0);
        let lone = &rail.items[0];
        assert!((lone.left + lone.width / 2.0 - 0.25 * 330.0).abs() < 0.1);
    }

    #[test]
    fn a_wide_track_separates_what_a_narrow_one_clusters() {
        let events = events();
        let refs = events.iter().collect::<Vec<_>>();
        // 5 minutes apart is ~1.04 pt per 300 s at 300 pt: clustered; at 20 000 pt: separate.
        assert_eq!(day_rail(&refs, Some("2026-08-20"), 300.0).items.len(), 2);
        assert_eq!(day_rail(&refs, Some("2026-08-20"), 20_000.0).items.len(), 4);
        assert!(day_rail(&refs, Some("2026-08-22"), 300.0).items.is_empty());
    }

    #[test]
    fn pills_show_only_their_count_when_the_track_is_crowded() {
        let events = events();
        let refs = events.iter().collect::<Vec<_>>();
        let rail = day_rail(&refs, Some("2026-08-20"), 140.0);
        assert!(!rail.full_pills);
        let pill = rail
            .items
            .iter()
            .find(|item| !item.is_marker())
            .expect("pill");
        assert_eq!(pill.width, COMPACT_PILL_WIDTH);
    }
}
