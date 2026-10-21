//! The 0–24 h day rail (`DESIGN.md` §6 Day rail): one coloured marker per event, the current one
//! taller with a white ring, and cluster pills where markers' 44 pt hit areas overlap on the
//! measured track.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, BoxShadow, ClickEvent, InteractiveElement as _, IntoElement,
    ParentElement as _, Role, StatefulInteractiveElement as _, Styled as _, Window, canvas, div,
    point, px,
};
use teslacam_core::copy;
use teslacam_core::day_timeline::DayTimelineCluster;
use teslacam_core::rail_layout::DayRail;

use super::components::category_marker;
use super::style::{AppTheme, Palette, TypeExt as _};

/// Height of the rail's hit row.
const TRACK_HEIGHT: f32 = 36.0;

/// What the rail reports.
pub enum RailClick {
    /// A lone marker: select its event.
    Event(String),
    /// A cluster pill: open or close its list.
    Cluster(DayTimelineCluster),
}

fn white_ring(palette: &Palette, control: &[BoxShadow]) -> Vec<BoxShadow> {
    let mut shadows = vec![BoxShadow {
        color: palette.surface,
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(0.0),
        spread_radius: px(2.0),
        inset: false,
    }];
    shadows.extend_from_slice(control);
    shadows
}

/// The rail of one day. `width` is the measured track width (written by the track's canvas; a
/// change refreshes the window so the clusters follow it).
pub fn day_rail(
    rail: &DayRail,
    selected: Option<&str>,
    expanded_cluster: Option<&str>,
    width: Rc<Cell<f32>>,
    on_click: impl Fn(RailClick, &mut Window, &mut App) + Clone + 'static,
    cx: &App,
) -> AnyElement {
    let theme = AppTheme::get(cx);
    let palette = theme.palette;
    let control = theme.control_shadow.clone();
    let track_width = width.get();
    let measure = {
        let width = width.clone();
        canvas(
            move |bounds, window, _| {
                let measured = bounds.size.width.as_f32();
                if (measured - width.get()).abs() > 0.5 {
                    width.set(measured);
                    window.refresh();
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full()
    };
    let marks = rail.loose_markers.iter().map(|marker| {
        let current = selected == Some(marker.event_id.as_str());
        let x = (marker.position_percent / 100.0) as f32 * track_width;
        let (mark_width, mark_height) = if current { (10.0, 24.0) } else { (8.0, 16.0) };
        div()
            .absolute()
            .left(px(x - mark_width / 2.0))
            .top(px((TRACK_HEIGHT - mark_height) / 2.0))
            .w(px(mark_width))
            .h(px(mark_height))
            .rounded(px(mark_width / 2.0))
            .bg(category_marker(marker.category, &palette))
            .when(current, |mark| mark.shadow(white_ring(&palette, &control)))
            .into_any_element()
    });
    let items = rail.items.iter().enumerate().map(|(index, item)| {
        let cluster = item.cluster.clone();
        let hit = div()
            .id(("rail-item", index))
            .absolute()
            .left(px(item.left as f32))
            .top_0()
            .w(px(item.width as f32))
            .h(px(TRACK_HEIGHT))
            .rounded(px(12.0))
            .cursor_pointer();
        let on_click = on_click.clone();
        if item.is_marker() {
            let marker = &cluster.markers[0];
            let event_id = marker.event_id.clone();
            return hit
                .role(Role::Button)
                .aria_label(copy::marker_label(
                    &marker.date_key,
                    &marker.time_label,
                    marker.category,
                ))
                .on_click(move |_: &ClickEvent, window, cx| {
                    on_click(RailClick::Event(event_id.clone()), window, cx)
                })
                .into_any_element();
        }
        let expanded = expanded_cluster == Some(cluster.id());
        let current = selected.is_some_and(|id| cluster.contains(id));
        let ring = if expanded || current {
            vec![BoxShadow {
                color: palette.drive,
                offset: point(px(0.0), px(0.0)),
                blur_radius: px(0.0),
                spread_radius: px(2.0),
                inset: false,
            }]
        } else {
            vec![BoxShadow {
                color: palette.line,
                offset: point(px(0.0), px(0.0)),
                blur_radius: px(0.0),
                spread_radius: px(1.0),
                inset: false,
            }]
        };
        let mut shadows = ring;
        shadows.extend_from_slice(&control);
        let pill = div()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(5.0))
            .w_full()
            .h(px(24.0))
            .pl(px(if rail.full_pills { 7.0 } else { 4.0 }))
            .pr(px(4.0))
            .rounded_full()
            .bg(palette.surface)
            .shadow(shadows)
            .child(
                div()
                    .flex()
                    .gap(px(2.0))
                    .children(cluster.markers.iter().take(3).map(|marker| {
                        div()
                            .w(px(4.0))
                            .h(px(10.0))
                            .rounded(px(2.0))
                            .bg(category_marker(marker.category, &palette))
                    })),
            )
            .when(rail.full_pills, |pill| {
                pill.child(
                    div()
                        .numeral()
                        .text_size(px(11.0))
                        .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                        .text_color(palette.ink)
                        .whitespace_nowrap()
                        .child(copy::cluster_range(
                            cluster.start_time_label(),
                            cluster.end_time_label(),
                        )),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .min_w(px(16.0))
                    .h(px(16.0))
                    .px(px(4.0))
                    .rounded_full()
                    .bg(palette.fill)
                    .numeral()
                    .text_size(px(11.0))
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .text_color(palette.ink)
                    .child(cluster.count().to_string()),
            );
        hit.role(Role::Button)
            .aria_label(copy::cluster_toggle_label(
                cluster.start_time_label(),
                cluster.end_time_label(),
                cluster.count(),
                expanded,
            ))
            .aria_expanded(expanded)
            .flex()
            .items_center()
            .child(pill)
            .on_click(move |_: &ClickEvent, window, cx| {
                on_click(RailClick::Cluster(cluster.clone()), window, cx)
            })
            .into_any_element()
    });
    div()
        .id("day-rail")
        .role(Role::Group)
        .aria_label(copy::DAY_MARKERS)
        .mt(px(6.0))
        .child(
            div()
                .relative()
                .h(px(TRACK_HEIGHT))
                .child(measure)
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .top(px(TRACK_HEIGHT / 2.0 - 2.0))
                        .h(px(4.0))
                        .rounded(px(2.0))
                        .bg(palette.fill),
                )
                .children(marks)
                .children(items),
        )
        .child(
            div()
                .mt(px(2.0))
                .flex()
                .justify_between()
                .numeral()
                .text_size(px(11.0))
                .font_weight(gpui_kit::FontWeight::MEDIUM)
                .text_color(palette.ink_3)
                .children(["0", "6", "12", "18", "24"].map(|hour| div().child(hour))),
        )
        .into_any_element()
}
