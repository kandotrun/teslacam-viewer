//! The top row (`DESIGN.md` §6 Event pill and details, Actions pill): what is playing, its details
//! popover, the view switch and the export action. Without a selected event it holds a placeholder.

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnchoredPositionMode, AnyElement, ClickEvent, Context, Entity, FontWeight,
    InteractiveElement as _, IntoElement, ParentElement as _, Render, RenderImage, Role,
    StatefulInteractiveElement as _, Styled as _, Subscription, TestSupportExt as _, Toggled,
    Window, anchored, canvas, deferred, div, img, point, px, relative,
};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use teslacam_core::copy;
use teslacam_core::event_copy::{EventDetails, event_details, event_pill};

use teslacam_core::export_state::{ExportAction, ExportState};
use teslacam_core::playback::{PlaybackInput, ViewMode};

use super::components::{
    ChipSize, KeyboardFocus as _, category_chip, glass, icon_button, popover_surface,
    primary_button, ring, segment_selected, text_button,
};
use super::style::{AppTheme, Palette, TypeExt as _, UI_FAMILY, icon, numeral_text, tabular};
use super::text::ja_text;
use crate::assets::{Icon, NUMERAL_FAMILY};
use crate::presentation::event_pill::{ITEM_GAP, PillWidths, pill_items};
use crate::presentation::floor_art::progress_ring_svg;
use crate::presentation::floor_layout::floor_layout;
use crate::store::{AppStore, StoreEvent};
use crate::ui::floor_phase;

/// The top row view.
pub struct TopRow {
    store: Entity<AppStore>,
    details_open: bool,
    technical_open: bool,
    export_reason_open: bool,
    /// The running export's popover (clock, bar, `中止`).
    export_progress_open: bool,
    /// The progress ring for (percent, scale bits).
    ring: Option<((u8, u32), Arc<RenderImage>)>,
    /// Measured width of the actions pill (the event pill gets the rest of the row).
    actions_width: Rc<Cell<f32>>,
    _subscriptions: Vec<Subscription>,
}

/// The export popovers are at least this wide, so a one-sentence message does not wrap into a
/// column under the narrow `もう一度` button.
const EXPORT_POPOVER_MIN_WIDTH: f32 = 240.0;

/// Space between the event pill and the actions pill.
const ROW_GAP: f32 = 10.0;
/// Horizontal padding of the event pill (8 left, 18 right).
const PILL_PADDING: f32 = 26.0;

/// The natural width of `text` set in one style.
fn text_width(
    window: &Window,
    text: &str,
    family: &'static str,
    size: f32,
    weight: f32,
    tabular_figures: bool,
) -> f32 {
    let mut font = gpui_kit::font(family);
    font.weight = FontWeight(weight);
    if tabular_figures {
        font.features = tabular();
    }
    let run = gpui_kit::TextRun {
        len: text.len(),
        font,
        color: gpui_kit::black(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(text.to_owned().into(), px(size), &[run], None)
        .width
        .as_f32()
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
                        this.export_progress_open = false;
                        cx.notify();
                    }
                }),
            ],
            store,
            details_open: false,
            technical_open: false,
            export_reason_open: false,
            export_progress_open: false,
            ring: None,
            actions_width: Rc::new(Cell::new(0.0)),
        }
    }

    /// Whether the details or an export popover is open.
    pub fn has_open_popover(&self) -> bool {
        self.details_open || self.export_reason_open || self.export_progress_open
    }

    /// Closes the details popover; whether one was open.
    pub fn close_popovers(&mut self, cx: &mut Context<Self>) -> bool {
        let open = self.details_open || self.export_reason_open || self.export_progress_open;
        self.details_open = false;
        self.export_reason_open = false;
        self.export_progress_open = false;
        cx.notify();
        open
    }

    /// Opens the running export's popover (development hook).
    pub fn open_export_progress(&mut self, cx: &mut Context<Self>) {
        self.export_progress_open = true;
        cx.notify();
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
                                    .child(ja_text(warning)),
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
                            .rounded(px(6.0))
                            .cursor_pointer()
                            .keyboard_focus(palette.surface, cx)
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

    /// The export action and what replaces it while exporting, after completion or after a
    /// failure (`DESIGN.md` §6 Actions pill, Mac form).
    /// The progress ring at `percent`, rasterised synchronously (an SVG image would load
    /// asynchronously for every new percentage and never show in this cached view).
    fn ring_image(
        &mut self,
        percent: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        let scale = window.scale_factor();
        if let Some((key, image)) = &self.ring
            && *key == (percent, scale.to_bits())
        {
            return Some(image.clone());
        }
        let colors = &AppTheme::get(cx).tokens.colors;
        let svg = progress_ring_svg(f32::from(percent) / 100.0, colors.line, colors.drive);
        let image = cx
            .svg_renderer()
            .render_single_frame(svg.as_bytes(), scale)
            .ok()?;
        if let Some((_, previous)) = self
            .ring
            .replace(((percent, scale.to_bits()), image.clone()))
        {
            cx.drop_image(previous, Some(window));
        }
        Some(image)
    }

    fn export_control(
        &mut self,
        state: ExportState,
        reason: Option<&'static str>,
        duration: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let ring = match state {
            ExportState::Running { progress } => {
                self.ring_image(copy::progress_percent(progress), window, cx)
            }
            _ => {
                self.export_progress_open = false;
                None
            }
        };
        let theme = AppTheme::get(cx);
        let (palette, tokens) = (theme.palette, theme.tokens);
        let hit = tokens.sizes.hit_target_desktop;
        let fill = palette.fill;
        // A popover right-aligned under the actions pill.
        let popover = |content: gpui_kit::Div, role: Role| {
            deferred(
                div().absolute().right(px(-6.0)).top(px(hit + 13.0)).child(
                    popover_surface(14.0, cx)
                        .id("export-popover")
                        .role(role)
                        .px(px(14.0))
                        .py(px(10.0))
                        .min_w(px(EXPORT_POPOVER_MIN_WIDTH))
                        .max_w(px(300.0))
                        .ui_text(12.0, 400.0)
                        .child(content),
                ),
            )
            .with_priority(1)
        };
        let action = |id: &'static str, glyph: Icon, label: &'static str, color| {
            div()
                .id(id)
                .role(Role::Button)
                .aria_label(label)
                .flex()
                .items_center()
                .gap(px(6.0))
                .h(px(hit))
                .px(px(14.0))
                .rounded_full()
                .ui_text(13.0, 600.0)
                .text_color(color)
                .whitespace_nowrap()
                .cursor_pointer()
                .hover(move |style| style.bg(fill))
                .keyboard_focus(palette.surface, cx)
                .child(icon(glyph, 18.0, color))
        };
        let control = div().relative().flex().items_center().gap(px(4.0));
        match state {
            ExportState::Running { progress } => {
                let button = div()
                    .id("export-progress")
                    .role(Role::Button)
                    .aria_label(copy::export_running_text(progress))
                    .aria_expanded(self.export_progress_open)
                    .flex()
                    .items_center()
                    .gap(px(6.0))
                    .h(px(hit))
                    .px(px(14.0))
                    .rounded_full()
                    .ui_text(13.0, 600.0)
                    .text_color(palette.drive)
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .hover(move |style| style.bg(fill))
                    .keyboard_focus(palette.surface, cx)
                    .children(ring.map(|ring| img(ring).size(px(20.0)).flex_none()))
                    .child(numeral_text(
                        &copy::export_running_text(progress),
                        14.0,
                        600.0,
                        None,
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.export_progress_open = !this.export_progress_open;
                        cx.notify();
                    }));
                let details = self.export_progress_open.then(|| {
                    popover(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.0))
                            .child(
                                div()
                                    .numeral()
                                    .text_size(px(13.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(palette.ink)
                                    .child(copy::export_progress_clock(progress, duration)),
                            )
                            .child(
                                div()
                                    .id("export-progress-bar")
                                    .role(Role::ProgressIndicator)
                                    .aria_label(copy::EXPORT_PROGRESS_LABEL)
                                    .aria_numeric_value(progress)
                                    .w(px(220.0))
                                    .h(px(6.0))
                                    .rounded_full()
                                    .bg(palette.fill)
                                    .child(
                                        div()
                                            .h_full()
                                            .w(relative(progress as f32))
                                            .rounded_full()
                                            .bg(palette.drive),
                                    ),
                            )
                            .child(div().ml(px(-14.0)).child(
                                text_button("export-abort", copy::EXPORT_ABORT, cx).on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.store.update(cx, |store, cx| {
                                            store.export(ExportAction::Abort, cx)
                                        });
                                    }),
                                ),
                            )),
                        Role::Status,
                    )
                });
                control
                    .child(button.test_support())
                    .children(details)
                    .into_any_element()
            }
            ExportState::Done { .. } => control
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .pl(px(10.0))
                        .pr(px(6.0))
                        .ui_text(13.0, 600.0)
                        .text_color(palette.saved_ink)
                        .whitespace_nowrap()
                        .child(icon(Icon::CircleCheck, 18.0, palette.saved_ink))
                        .child(copy::EXPORT_DONE),
                )
                .child(
                    primary_button(
                        "export-reveal",
                        Icon::FolderOpen,
                        copy::EXPORT_REVEAL,
                        {
                            let store = self.store.clone();
                            move |_, _, cx| {
                                store
                                    .update(cx, |store, cx| store.export(ExportAction::Reveal, cx));
                            }
                        },
                        cx,
                    )
                    .h(px(hit))
                    .px(px(14.0))
                    .ui_text(13.0, 600.0),
                )
                .child(
                    icon_button("export-close", Icon::X, copy::EXPORT_CLOSE, true, cx).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.store
                                .update(cx, |store, cx| store.export(ExportAction::Close, cx));
                        }),
                    ),
                )
                .into_any_element(),
            ExportState::Failed { message } => control
                .child(
                    action(
                        "export-retry",
                        Icon::CircleAlert,
                        copy::EXPORT_RETRY,
                        palette.ink,
                    )
                    .child(copy::EXPORT_RETRY)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.store
                            .update(cx, |store, cx| store.export(ExportAction::Open, cx));
                    })),
                )
                .child(popover(
                    div().text_color(palette.warning).child(ja_text(message)),
                    Role::Alert,
                ))
                .into_any_element(),
            state => {
                let unavailable = reason.is_some();
                let color = if unavailable {
                    palette.ink_3
                } else {
                    palette.ink
                };
                let button = action("export", Icon::Download, copy::EXPORT_DIALOG_TITLE, color)
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
                let note = match (&state, reason) {
                    (_, Some(reason)) if self.export_reason_open => Some((reason, Role::Tooltip)),
                    (ExportState::Cancelled, _) => Some((copy::EXPORT_ABORTED, Role::Status)),
                    _ => None,
                };
                control
                    .child(button.test_support())
                    .children(note.map(|(text, role)| {
                        popover(div().text_color(palette.ink_2).child(ja_text(text)), role)
                    }))
                    .into_any_element()
            }
        }
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
            .keyboard_focus(palette.fill, cx)
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
            let floor = floor_layout(
                viewport.width.as_f32(),
                viewport.height.as_f32(),
                floor_phase(store),
                tokens,
            );
            let (compact, row_width) =
                (floor.compact, floor.top_row.map_or(0.0, |rect| rect.width));
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
                        (compact, row_width),
                        session.export.state().clone(),
                        session.timeline.duration(),
                    )
                })
        };
        let Some((
            pill,
            details,
            view_mode,
            has_representative,
            export_reason,
            (compact, row_width),
            export_state,
            duration,
        )) = snapshot
        else {
            return self.placeholder(cx);
        };
        // Items drop whole from the right, detection first, then place (DESIGN.md §6).
        let shown = {
            let measure =
                |text: &str, family: &'static str, size: f32, weight: f32, tabular: bool| {
                    text_width(window, text, family, size, weight, tabular)
                };
            let numeral_m = &tokens.type_scale.numeral_m;
            let chip = 12.0
                + 14.0
                + 4.0
                + measure(
                    copy::category_label(pill.category),
                    UI_FAMILY,
                    12.0,
                    600.0,
                    false,
                )
                + 12.0;
            let clock = measure(
                &pill.start_clock,
                NUMERAL_FAMILY,
                numeral_m.size,
                f32::from(numeral_m.weight),
                true,
            ) + 4.0
                + measure(copy::EVENT_START_SUFFIX, UI_FAMILY, 15.0, 600.0, false);
            let widths = PillWidths {
                fixed: PILL_PADDING + chip + ITEM_GAP + clock,
                place: pill
                    .location
                    .as_ref()
                    .map(|location| 15.0 + 5.0 + measure(location, UI_FAMILY, 13.0, 400.0, false)),
                detection: pill
                    .detection
                    .as_ref()
                    .filter(|_| !compact)
                    .map(|detection| {
                        15.0 + 6.0
                            + measure(&detection.lead, UI_FAMILY, 13.0, 400.0, false)
                            + measure(&detection.time, NUMERAL_FAMILY, 15.0, 600.0, true)
                            + measure(&detection.tail, UI_FAMILY, 13.0, 400.0, false)
                    }),
            };
            let available = row_width - self.actions_width.get() - ROW_GAP;
            pill_items(available, widths)
        };
        let ink = palette.ink;
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
            .gap(px(ITEM_GAP))
            .min_w_0()
            .h(px(tokens.sizes.topbar_height))
            .pl(px(8.0))
            .pr(px(PILL_PADDING - 8.0))
            .overflow_hidden()
            .cursor_pointer()
            .hover(move |style| style.shadow(hover_shadow.clone()))
            .keyboard_focus(palette.floor, cx)
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
            .when_some(
                pill.location.clone().filter(|_| shown.place),
                |row, location| {
                    row.child(separator(&palette)).child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.0))
                            .flex_none()
                            .ui_text(13.0, 400.0)
                            .text_color(palette.ink_2)
                            .child(icon(Icon::MapPin, 15.0, palette.ink_2))
                            .child(div().whitespace_nowrap().child(location)),
                    )
                },
            )
            .when_some(
                pill.detection.clone().filter(|_| shown.detection),
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
        let measured = self.actions_width.clone();
        let actions = glass(999.0, cx)
            .relative()
            .child(
                canvas(
                    move |bounds, window, _| {
                        let width = bounds.size.width.as_f32();
                        if (width - measured.get()).abs() > 0.5 {
                            measured.set(width);
                            window.refresh();
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
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
            .child(self.export_control(export_state, export_reason, duration, window, cx));
        div()
            .size_full()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(ROW_GAP))
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
