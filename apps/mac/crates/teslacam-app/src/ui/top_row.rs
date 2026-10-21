//! The top row (`DESIGN.md` §6 Event pill and details, Actions pill): what is playing, its details
//! popover, the view switch and the export action. Without a selected event it holds a placeholder.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnchoredPositionMode, AnyElement, ClickEvent, Context, Entity, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, Role,
    StatefulInteractiveElement as _, Styled as _, Subscription, Toggled, Window, anchored,
    deferred, div, point, px, relative,
};
use teslacam_core::copy;
use teslacam_core::event_copy::{EventDetails, event_details, event_pill};
use teslacam_core::export_state::ExportAction;
use teslacam_core::playback::{PlaybackInput, ViewMode};

use super::components::{ChipSize, category_chip, glass, popover_surface, ring, segment_selected};
use super::style::{AppTheme, Palette, TypeExt as _, icon, numeral_text};
use crate::assets::Icon;
use crate::presentation::floor_layout::floor_layout;
use crate::store::{AppStore, StoreEvent};
use crate::ui::floor_phase;

/// The top row view.
pub struct TopRow {
    store: Entity<AppStore>,
    details_open: bool,
    technical_open: bool,
    export_reason_open: bool,
    _subscriptions: Vec<Subscription>,
}

impl TopRow {
    /// A top row reading `store`.
    pub fn new(store: Entity<AppStore>, cx: &mut Context<Self>) -> TopRow {
        TopRow {
            _subscriptions: vec![
                cx.observe(&store, |_, _, cx| cx.notify()),
                cx.subscribe(&store, |this, _, event, cx| {
                    if *event == StoreEvent::SessionChanged {
                        this.details_open = false;
                        this.technical_open = false;
                        this.export_reason_open = false;
                        cx.notify();
                    }
                }),
            ],
            store,
            details_open: false,
            technical_open: false,
            export_reason_open: false,
        }
    }

    /// Closes the details popover; whether one was open.
    pub fn close_popovers(&mut self, cx: &mut Context<Self>) -> bool {
        let open = self.details_open || self.export_reason_open;
        self.details_open = false;
        self.export_reason_open = false;
        cx.notify();
        open
    }

    /// Opens the details popover (and its `技術情報` disclosure).
    pub fn open_details(&mut self, technical: bool, cx: &mut Context<Self>) {
        self.details_open = true;
        self.technical_open = technical;
        cx.notify();
    }
}

fn separator(palette: &Palette) -> gpui_kit::Div {
    div().flex_none().w(px(1.0)).h(px(18.0)).bg(palette.line)
}

fn details_popover(
    details: &EventDetails,
    technical_open: bool,
    on_toggle_technical: impl Fn(&ClickEvent, &mut Window, &mut gpui_kit::App) + 'static,
    on_close: impl Fn(&gpui_kit::MouseDownEvent, &mut Window, &mut gpui_kit::App) + 'static,
    cx: &gpui_kit::App,
) -> AnyElement {
    let theme = AppTheme::get(cx);
    let palette = theme.palette;
    let numeric =
        |label: &str| label == copy::DETAILS_COORDINATES || label == copy::DETAILS_RECORDING_START;
    popover_surface(theme.tokens.radii.dialog, cx)
        .id("event-details")
        .role(Role::Dialog)
        .aria_label(copy::DETAILS_REGION)
        .occlude()
        .w(px(360.0))
        .px(px(18.0))
        .py(px(16.0))
        .flex()
        .flex_col()
        .gap(px(10.0))
        .on_mouse_down_out(on_close)
        .children(details.rows.iter().map(|row| {
            let value = if numeric(row.label) {
                div()
                    .numeral()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .child(row.value.clone())
                    .into_any_element()
            } else {
                numeral_text(&row.value, 14.0, 500.0, None).into_any_element()
            };
            div()
                .flex()
                .gap(px(12.0))
                .child(
                    div()
                        .flex_none()
                        .w(px(136.0))
                        .ui_text(12.0, 400.0)
                        .line_height(relative(1.5))
                        .text_color(palette.ink_3)
                        .child(row.label),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .ui_text(13.0, 500.0)
                        .line_height(relative(1.4))
                        .text_color(palette.ink)
                        .child(value)
                        .when_some(row.warning.clone(), |value, warning| {
                            value.child(
                                div()
                                    .ui_text(12.0, 400.0)
                                    .text_color(palette.warning)
                                    .child(warning),
                            )
                        }),
                )
        }))
        .when_some(details.technical.clone(), |popover, reason| {
            popover.child(
                div()
                    .flex()
                    .flex_col()
                    .ui_text(12.0, 400.0)
                    .text_color(palette.ink_2)
                    .child(
                        div()
                            .id("technical")
                            .role(Role::Button)
                            .aria_label(copy::DETAILS_TECHNICAL)
                            .aria_expanded(technical_open)
                            .flex()
                            .items_center()
                            .gap(px(6.0))
                            .min_h(px(28.0))
                            .cursor_pointer()
                            .child(div().text_size(px(9.0)).child(if technical_open {
                                "\u{25BC}"
                            } else {
                                "\u{25B6}"
                            }))
                            .child(copy::DETAILS_TECHNICAL)
                            .on_click(on_toggle_technical),
                    )
                    .when(technical_open, |disclosure| {
                        disclosure.child(
                            div()
                                .mt(px(4.0))
                                .px(px(10.0))
                                .py(px(8.0))
                                .rounded(px(10.0))
                                .bg(palette.fill)
                                .child(reason),
                        )
                    }),
            )
        })
        .into_any_element()
}

impl TopRow {
    fn placeholder(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = AppTheme::get(cx);
        let palette = theme.palette;
        div()
            .flex()
            .child(
                glass(theme.tokens.radii.panel, cx)
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .min_h(px(46.0))
                    .px(px(22.0))
                    .py(px(10.0))
                    .child(
                        div()
                            .ui_text(15.0, 600.0)
                            .text_color(palette.ink)
                            .child(copy::NO_SELECTION_TITLE),
                    )
                    .child(
                        div()
                            .ui_text(12.0, 400.0)
                            .text_color(palette.ink_2)
                            .child(copy::NO_SELECTION_DESCRIPTION),
                    ),
            )
            .into_any_element()
    }

    fn view_option(&self, mode: ViewMode, current: ViewMode, cx: &mut Context<Self>) -> AnyElement {
        let palette = AppTheme::get(cx).palette;
        let (glyph, label, id) = match mode {
            ViewMode::Vehicle => (Icon::Car, copy::VIEW_VEHICLE, "view-vehicle"),
            ViewMode::Grid => (Icon::LayoutGrid, copy::VIEW_GRID, "view-grid"),
            ViewMode::Representative => {
                (Icon::Film, copy::VIEW_REPRESENTATIVE, "view-representative")
            }
        };
        let checked = mode == current;
        let ink = palette.ink;
        let item = div()
            .id(id)
            .role(Role::RadioButton)
            .aria_label(label)
            .aria_toggled(if checked {
                Toggled::True
            } else {
                Toggled::False
            })
            .flex()
            .items_center()
            .gap(px(6.0))
            .h(px(32.0))
            .px(px(12.0))
            .rounded_full()
            .ui_text(13.0, 500.0)
            .text_color(palette.ink_2)
            .whitespace_nowrap()
            .cursor_pointer()
            .child(icon(
                glyph,
                18.0,
                if checked { palette.ink } else { palette.ink_2 },
            ))
            .child(label)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.store.update(cx, |store, cx| {
                    store.playback(PlaybackInput::SetViewMode(mode), cx)
                });
            }));
        if checked {
            segment_selected(item, cx).into_any_element()
        } else {
            item.hover(move |style| style.text_color(ink))
                .into_any_element()
        }
    }
}

impl Render for TopRow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (tokens, palette) = {
            let theme = AppTheme::get(cx);
            (theme.tokens, theme.palette)
        };
        let viewport = window.viewport_size();
        let snapshot = {
            let store = self.store.read(cx);
            let compact = floor_layout(
                viewport.width.as_f32(),
                viewport.height.as_f32(),
                floor_phase(store),
                tokens,
            )
            .compact;
            store
                .session()
                .zip(store.selected_event())
                .map(|(session, event)| {
                    (
                        event_pill(event, &session.timeline),
                        event_details(event, &session.timeline),
                        session.playback.view_mode(),
                        event.representative.is_some(),
                        session.export.unavailable_reason(),
                        compact,
                    )
                })
        };
        let Some((pill, details, view_mode, has_representative, export_reason, compact)) = snapshot
        else {
            return self.placeholder(cx);
        };
        let (ink, fill) = (palette.ink, palette.fill);
        let float = AppTheme::get(cx).float_shadow.clone();
        let hover_shadow = {
            let mut shadows = ring(palette.glass, 0.0, ink, 2.0);
            shadows.extend(float);
            shadows
        };
        let event_pill = glass(999.0, cx)
            .id("event-pill")
            .role(Role::Button)
            .aria_label(copy::DETAILS_REGION)
            .aria_expanded(self.details_open)
            .flex()
            .items_center()
            .gap(px(14.0))
            .min_w_0()
            .h(px(tokens.sizes.topbar_height))
            .pl(px(8.0))
            .pr(px(18.0))
            .overflow_hidden()
            .cursor_pointer()
            .hover(move |style| style.shadow(hover_shadow.clone()))
            .child(category_chip(pill.category, ChipSize::Large, cx))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_baseline()
                    .ui_text(15.0, 600.0)
                    .text_color(palette.ink)
                    .whitespace_nowrap()
                    .child(
                        div()
                            .type_role(&tokens.type_scale.numeral_m)
                            .child(pill.start_clock.clone()),
                    )
                    .child(div().ml(px(4.0)).child(copy::EVENT_START_SUFFIX)),
            )
            .when_some(pill.location.clone(), |row, location| {
                row.child(separator(&palette)).child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(5.0))
                        .min_w_0()
                        .flex_shrink(1.0)
                        .ui_text(13.0, 400.0)
                        .text_color(palette.ink_2)
                        .child(icon(Icon::MapPin, 15.0, palette.ink_2))
                        .child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .text_ellipsis()
                                .whitespace_nowrap()
                                .child(location),
                        ),
                )
            })
            .when_some(
                pill.detection.clone().filter(|_| !compact),
                |row, detection| {
                    row.child(separator(&palette)).child(
                        div()
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap(px(6.0))
                            .ui_text(13.0, 400.0)
                            .text_color(palette.ink_2)
                            .whitespace_nowrap()
                            .child(icon(Icon::Eye, 15.0, palette.ink_2))
                            .child(
                                div()
                                    .flex()
                                    .items_baseline()
                                    .child(detection.lead)
                                    .child(
                                        div()
                                            .numeral()
                                            .text_size(px(15.0))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(palette.sentry_ink)
                                            .child(detection.time),
                                    )
                                    .child(detection.tail),
                            ),
                    )
                },
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.details_open = !this.details_open;
                cx.notify();
            }));
        let entity = cx.entity().downgrade();
        let close_entity = entity.clone();
        let popover = self.details_open.then(|| {
            deferred(
                anchored()
                    .position_mode(AnchoredPositionMode::Local)
                    .position(point(px(0.0), px(tokens.sizes.topbar_height + 8.0)))
                    .snap_to_window_with_margin(px(16.0))
                    .child(details_popover(
                        &details,
                        self.technical_open,
                        move |_, _, cx| {
                            entity
                                .update(cx, |this, cx| {
                                    this.technical_open = !this.technical_open;
                                    cx.notify();
                                })
                                .ok();
                        },
                        move |_, _, cx| {
                            close_entity
                                .update(cx, |this, cx| {
                                    this.details_open = false;
                                    cx.notify();
                                })
                                .ok();
                        },
                        cx,
                    )),
            )
            .with_priority(1)
        });
        let mut modes = vec![ViewMode::Vehicle, ViewMode::Grid];
        if has_representative {
            modes.push(ViewMode::Representative);
        }
        let current = if view_mode == ViewMode::Representative && !has_representative {
            ViewMode::Vehicle
        } else {
            view_mode
        };
        let unavailable = export_reason.is_some();
        let export = div()
            .id("export")
            .role(Role::Button)
            .aria_label(copy::EXPORT_DIALOG_TITLE)
            .flex()
            .items_center()
            .gap(px(6.0))
            .h(px(tokens.sizes.hit_target_desktop))
            .px(px(14.0))
            .rounded_full()
            .ui_text(13.0, 600.0)
            .text_color(if unavailable {
                palette.ink_3
            } else {
                palette.ink
            })
            .whitespace_nowrap()
            .cursor_pointer()
            .hover(move |style| style.bg(fill))
            .child(icon(
                Icon::Download,
                18.0,
                if unavailable {
                    palette.ink_3
                } else {
                    palette.ink
                },
            ))
            .child(copy::EXPORT)
            .on_click(cx.listener(move |this, _, _, cx| {
                if unavailable {
                    this.export_reason_open = !this.export_reason_open;
                    cx.notify();
                } else {
                    this.store
                        .update(cx, |store, cx| store.export(ExportAction::Open, cx));
                }
            }));
        let reason_popover = export_reason
            .filter(|_| self.export_reason_open)
            .map(|reason| {
                deferred(
                    anchored()
                        .position_mode(AnchoredPositionMode::Local)
                        .position(point(px(0.0), px(tokens.sizes.topbar_height + 8.0)))
                        .snap_to_window_with_margin(px(16.0))
                        .child(
                            popover_surface(14.0, cx)
                                .px(px(14.0))
                                .py(px(10.0))
                                .max_w(px(300.0))
                                .ui_text(12.0, 400.0)
                                .text_color(palette.ink_2)
                                .child(reason),
                        ),
                )
            });
        let actions = glass(999.0, cx)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(4.0))
            .h(px(tokens.sizes.topbar_height))
            .px(px(6.0))
            .child(
                div()
                    .id("view-modes")
                    .role(Role::RadioGroup)
                    .aria_label(copy::VIEW_GROUP)
                    .flex()
                    .gap(px(2.0))
                    .p(px(2.0))
                    .rounded_full()
                    .bg(palette.fill)
                    .children(
                        modes
                            .into_iter()
                            .map(|mode| self.view_option(mode, current, cx)),
                    ),
            )
            .child(div().relative().child(export).children(reason_popover));
        div()
            .size_full()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(10.0))
            .child(
                div()
                    .relative()
                    .flex()
                    .min_w_0()
                    .child(event_pill)
                    .children(popover),
            )
            .child(actions)
            .into_any_element()
    }
}
