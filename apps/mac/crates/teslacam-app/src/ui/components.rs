//! Small building blocks shared by the views: floating glass, pills, chips, icon buttons and
//! segmented tracks (`DESIGN.md` §4, §6).

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    App, BoxShadow, ClickEvent, Div, ElementId, FocusHandle, Hsla, InteractiveElement,
    ParentElement as _, Role, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _,
    Window, div, point, px,
};
use teslacam_core::camera::ClipCategory;
use teslacam_core::copy;

use super::style::{AppTheme, Palette, TypeExt as _, icon};
use crate::assets::Icon;

/// A floating layer: glass, one soft shadow, a hairline border (`radius` 22 for panels).
pub fn glass(radius: f32, cx: &App) -> Div {
    let theme = AppTheme::get(cx);
    div()
        .bg(theme.palette.glass)
        .rounded(px(radius))
        .border_1()
        .border_color(theme.palette.ink.opacity(0.05))
        .shadow(theme.float_shadow.clone())
}

/// A popover (details, date picker, notices): like [`glass`] but on the opaque `surface`, because
/// without GPUI blur a translucent layer lets video and labels behind it show through.
pub fn popover_surface(radius: f32, cx: &App) -> Div {
    let theme = AppTheme::get(cx);
    glass(radius, cx).bg(theme.palette.surface)
}

/// The icon of a category (never colour alone: icon + word + colour).
pub fn category_icon(category: ClipCategory) -> Icon {
    match category {
        ClipCategory::Sentry => Icon::Shield,
        ClipCategory::Saved => Icon::Bookmark,
        ClipCategory::Recent => Icon::Route,
    }
}

/// `(text, background)` of a category chip.
pub fn category_colors(category: ClipCategory, palette: &Palette) -> (Hsla, Hsla) {
    match category {
        ClipCategory::Sentry => (palette.sentry_ink, palette.sentry_soft),
        ClipCategory::Saved => (palette.saved_ink, palette.saved_soft),
        ClipCategory::Recent => (palette.recent, palette.fill),
    }
}

/// The marker colour of a category on the day rail.
pub fn category_marker(category: ClipCategory, palette: &Palette) -> Hsla {
    match category {
        ClipCategory::Sentry => palette.sentry,
        ClipCategory::Saved => palette.saved,
        ClipCategory::Recent => palette.recent_marker,
    }
}

/// Chip sizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChipSize {
    /// 22 pt, short words (list rows).
    Regular,
    /// 30 pt, full words (event pill).
    Large,
}

/// A category chip: icon, word and colour.
pub fn category_chip(category: ClipCategory, size: ChipSize, cx: &App) -> Div {
    let palette = &AppTheme::get(cx).palette;
    let (text, background) = category_colors(category, palette);
    let (height, padding, font, icon_size, label) = match size {
        ChipSize::Regular => (22.0, 8.0, 11.0, 13.0, copy::category_short_label(category)),
        ChipSize::Large => (30.0, 12.0, 12.0, 14.0, copy::category_label(category)),
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(4.0))
        .h(px(height))
        .px(px(padding))
        .rounded_full()
        .bg(background)
        .text_color(text)
        .ui_text(font, 600.0)
        .child(icon(category_icon(category), icon_size, text))
        .child(label)
}

/// The keyboard focus ring (`DESIGN.md` §2: a 2 pt `drive` outline 3 pt outside the control). The
/// gap is painted in `gap`, the colour the control sits on.
pub fn focus_ring(gap: Hsla, cx: &App) -> Vec<BoxShadow> {
    ring(gap, 3.0, AppTheme::get(cx).palette.drive, 2.0)
}

/// Keyboard reachability for a control: a tab stop that a click also focuses (like a button on the
/// web, so the theater shortcuts wait while it has focus), with [`focus_ring`] while focused from
/// the keyboard.
pub trait KeyboardFocus: InteractiveElement + Sized {
    /// A tab stop in document order with the ring on `gap`.
    fn keyboard_focus(self, gap: Hsla, cx: &App) -> Self {
        let ring = focus_ring(gap, cx);
        self.tab_index(0)
            .focus_visible(move |style| style.shadow(ring))
    }

    /// Like [`KeyboardFocus::keyboard_focus`] with a handle owned by the view (for moving focus
    /// with the arrow keys); the handle must be a tab stop.
    fn keyboard_focus_handle(self, handle: &FocusHandle, gap: Hsla, cx: &App) -> Self {
        let ring = focus_ring(gap, cx);
        self.track_focus(handle)
            .focus_visible(move |style| style.shadow(ring))
    }
}

impl<E: InteractiveElement> KeyboardFocus for E {}

/// A round 36 pt icon button (`ink-2`, `fill` on hover) with an accessible name.
pub fn icon_button(
    id: impl Into<ElementId>,
    glyph: Icon,
    label: impl Into<SharedString>,
    enabled: bool,
    cx: &App,
) -> Stateful<Div> {
    let palette = &AppTheme::get(cx).palette;
    let size = AppTheme::get(cx).tokens.sizes.hit_target_desktop;
    let (fill, ink) = (palette.fill, palette.ink);
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded_full()
        .child(icon(glyph, 20.0, palette.ink_2))
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(move |style| style.bg(fill).text_color(ink))
                .keyboard_focus(palette.surface, cx)
        })
        .when(!enabled, |button| button.opacity(0.35))
}

/// The selected item of a segmented track: `surface` with `shadow-control`.
pub fn segment_selected(item: Stateful<Div>, cx: &App) -> Stateful<Div> {
    let theme = AppTheme::get(cx);
    item.bg(theme.palette.surface)
        .text_color(theme.palette.ink)
        .shadow(theme.control_shadow.clone())
}

/// A primary pill button (50 pt, `drive`, white text) like `フォルダを選ぶ`.
pub fn primary_button(
    id: impl Into<ElementId>,
    glyph: Icon,
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &App,
) -> Stateful<Div> {
    let palette = &AppTheme::get(cx).palette;
    let hover = palette.drive_hover;
    let label = label.into();
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label.clone())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(8.0))
        .h(px(50.0))
        .px(px(26.0))
        .rounded_full()
        .bg(palette.drive)
        .text_color(palette.surface)
        .ui_text(15.0, 600.0)
        .cursor_pointer()
        .hover(move |style| style.bg(hover))
        .keyboard_focus(palette.surface, cx)
        .child(icon(glyph, 18.0, palette.surface))
        .child(label)
        .on_click(on_click)
}

/// A text button (`ink`, 600, `fill` on hover).
pub fn text_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    cx: &App,
) -> Stateful<Div> {
    let palette = &AppTheme::get(cx).palette;
    let fill = palette.fill;
    let label = label.into();
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label.clone())
        .flex()
        .flex_none()
        .items_center()
        .h(px(AppTheme::get(cx).tokens.sizes.hit_target_desktop))
        .px(px(14.0))
        .rounded_full()
        .ui_text(13.0, 600.0)
        .text_color(palette.ink)
        .cursor_pointer()
        .hover(move |style| style.bg(fill))
        .keyboard_focus(palette.surface, cx)
        .child(label)
}

/// An inset hairline (`inset 0 0 0 1px`) in `color`.
pub fn hairline(color: Hsla) -> BoxShadow {
    BoxShadow {
        color,
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(0.0),
        spread_radius: px(1.0),
        inset: true,
    }
}

/// A ring `width` wide drawn `gap` outside the element (CSS `0 0 0 gap floor, 0 0 0 gap+width color`).
pub fn ring(gap_color: Hsla, gap: f32, color: Hsla, width: f32) -> Vec<BoxShadow> {
    vec![
        BoxShadow {
            color,
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(gap + width),
            inset: false,
        },
        BoxShadow {
            color: gap_color,
            offset: point(px(0.0), px(0.0)),
            blur_radius: px(0.0),
            spread_radius: px(gap),
            inset: false,
        },
    ]
}
