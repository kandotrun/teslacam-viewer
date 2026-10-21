//! The date picker popover under the day title: a Sunday-first month grid where recorded days are
//! set in ink with a blue dot and the day on screen is filled with `drive`.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, App, ClickEvent, FontWeight, InteractiveElement as _, IntoElement,
    ParentElement as _, Role, StatefulInteractiveElement as _, Styled as _, Window, div, px,
};
use teslacam_core::calendar::MonthGrid;
use teslacam_core::copy;

use super::components::{KeyboardFocus as _, icon_button, popover_surface};
use super::style::{AppTheme, TypeExt as _};
use crate::assets::Icon;

/// What the picker asks for.
pub enum PickerAction {
    /// Show the month `delta` months away.
    Month(i32),
    /// Choose this `YYYY-MM-DD`.
    Day(String),
    /// Close without choosing.
    Close,
}

const CELL: f32 = 38.0;

/// The popover for `grid`.
pub fn date_picker(
    grid: &MonthGrid,
    on_action: impl Fn(PickerAction, &mut Window, &mut App) + Clone + 'static,
    cx: &App,
) -> AnyElement {
    let theme = AppTheme::get(cx);
    let palette = theme.palette;
    let month_button = |id: &'static str, glyph, label, delta: i32| {
        let on_action = on_action.clone();
        icon_button(id, glyph, label, true, cx).on_click(move |_: &ClickEvent, window, cx| {
            on_action(PickerAction::Month(delta), window, cx)
        })
    };
    let close = on_action.clone();
    popover_surface(18.0, cx)
        .id("date-picker")
        .role(Role::Dialog)
        .aria_label(copy::DATE_LABEL)
        .occlude()
        .w(px(CELL * 7.0 + 28.0))
        .p(px(14.0))
        .on_mouse_down_out(move |_, window, cx| close(PickerAction::Close, window, cx))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .mb(px(6.0))
                .child(month_button(
                    "previous-month",
                    Icon::ChevronLeft,
                    copy::PREVIOUS_MONTH,
                    -1,
                ))
                .child(
                    div()
                        .ui_text(15.0, 600.0)
                        .text_color(palette.ink)
                        .child(grid.month.title()),
                )
                .child(month_button(
                    "next-month",
                    Icon::ChevronRight,
                    copy::NEXT_MONTH,
                    1,
                )),
        )
        .child(div().flex().children(copy::WEEKDAY_INITIALS.map(|initial| {
            div()
                .w(px(CELL))
                .flex()
                .justify_center()
                .ui_text(11.0, 600.0)
                .text_color(palette.ink_3)
                .child(initial)
        })))
        .children(grid.weeks.iter().enumerate().map(|(week_index, week)| {
            div()
                .flex()
                .children(week.iter().enumerate().map(|(column, day)| {
                    let Some(day) = day else {
                        return div().w(px(CELL)).h(px(34.0)).into_any_element();
                    };
                    let date_key = day.date_key.clone();
                    let on_action = on_action.clone();
                    let (text, weight) = match (day.selected, day.recorded) {
                        (true, _) => (palette.surface, FontWeight::SEMIBOLD),
                        (false, true) => (palette.ink, FontWeight::SEMIBOLD),
                        (false, false) => (palette.ink_3, FontWeight::MEDIUM),
                    };
                    let fill = palette.fill;
                    div()
                        .id(("day", week_index * 7 + column))
                        .role(Role::Button)
                        .aria_label(day.date_key.clone())
                        .aria_selected(day.selected)
                        .w(px(CELL))
                        .h(px(34.0))
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .rounded_full()
                        .cursor_pointer()
                        .keyboard_focus(palette.surface, cx)
                        .when(day.selected, |cell| cell.bg(palette.drive))
                        .when(!day.selected, |cell| {
                            cell.hover(move |style| style.bg(fill))
                        })
                        .child(
                            div()
                                .numeral()
                                .text_size(px(14.0))
                                .font_weight(weight)
                                .text_color(text)
                                .child(day.day.to_string()),
                        )
                        .child(
                            div()
                                .size(px(4.0))
                                .rounded_full()
                                .when(day.recorded && !day.selected, |dot| dot.bg(palette.drive)),
                        )
                        .on_click(move |_: &ClickEvent, window, cx| {
                            on_action(PickerAction::Day(date_key.clone()), window, cx)
                        })
                        .into_any_element()
                }))
        }))
        .into_any_element()
}
