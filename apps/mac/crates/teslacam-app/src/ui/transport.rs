//! The transport floating under the stage (`DESIGN.md` §6 Transport): previous / next event, the
//! 58 pt play button, the Barlow clock, the scrubber with its marks, ±10 s and the rate pill, and
//! the playback message.
//!
//! It reads the pure session for intent, rate, marks and message, and the media for the position
//! (which moves every frame while playing; the stage's frame loop redraws the window then).

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Bounds, Context, DispatchPhase, Entity, FontWeight, InteractiveElement as _,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    ParentElement as _, Pixels, Render, Role, StatefulInteractiveElement as _, Styled as _,
    Subscription, TestSupportExt as _, Window, canvas, deferred, div, point, px, relative,
};
use teslacam_core::app_state::Step;
use teslacam_core::copy;
use teslacam_core::detection::sentry_detection;
use teslacam_core::playback::{PlaybackInput, PlaybackRate, ViewMode};
use teslacam_media::host_now;

use super::components::{KeyboardFocus as _, icon_button, popover_surface};
use super::style::{AppTheme, Palette, TypeExt as _, icon};
use crate::assets::Icon;
use crate::presentation::transport::{
    RAIL_INSET, ScrubberMarks, fraction, scrubber_marks, seconds_at, slider_key_target,
};
use crate::store::{AppStore, StoreEvent};

/// Track geometry inside the 46 pt scrubber (web `transport.css`).
const SCRUBBER_HEIGHT: f32 = 46.0;
const TRACK_TOP: f32 = 20.0;
const TRACK_HEIGHT: f32 = 6.0;
const THUMB: f32 = 20.0;
/// Width of the boxes that centre a mark's label on its position.
const MARK_BOX: f32 = 120.0;

/// The transport view.
pub struct TransportView {
    store: Entity<AppStore>,
    rate_open: bool,
    scrubber: Rc<Cell<Option<Bounds<Pixels>>>>,
    dragging: bool,
    _subscriptions: Vec<Subscription>,
}

/// What the transport shows.
struct Snapshot {
    position: f64,
    duration: f64,
    playing: bool,
    rate: PlaybackRate,
    marks: ScrubberMarks,
    message: Option<&'static str>,
    older: bool,
    newer: bool,
}

impl TransportView {
    /// A transport reading `store`.
    pub fn new(store: Entity<AppStore>, cx: &mut Context<Self>) -> TransportView {
        TransportView {
            _subscriptions: vec![
                cx.observe(&store, |_, _, cx| cx.notify()),
                cx.subscribe(&store, |this, _, event, cx| {
                    if *event == StoreEvent::SessionChanged {
                        this.rate_open = false;
                        this.dragging = false;
                        cx.notify();
                    }
                }),
            ],
            store,
            rate_open: false,
            scrubber: Rc::default(),
            dragging: false,
        }
    }

    /// Opens the rate menu (development hook).
    pub fn open_rate_menu(&mut self, cx: &mut Context<Self>) {
        self.rate_open = true;
        cx.notify();
    }

    /// Whether the rate menu is open.
    pub fn has_open_popover(&self) -> bool {
        self.rate_open
    }

    /// Closes the rate menu; whether it was open.
    pub fn close_popovers(&mut self, cx: &mut Context<Self>) -> bool {
        let open = self.rate_open;
        self.rate_open = false;
        cx.notify();
        open
    }

    fn snapshot(&self, cx: &Context<Self>) -> Option<Snapshot> {
        let store = self.store.read(cx);
        let session = store.session()?;
        let event = store.selected_event()?;
        let workspace = store.workspace()?;
        let playback = &session.playback;
        let position = store.media().map_or(playback.position(), |media| {
            media.read(cx).position(host_now())
        });
        let starts: Vec<f64> = session
            .timeline
            .segments()
            .iter()
            .map(|segment| segment.start)
            .collect();
        let detection = sentry_detection(event, &session.timeline);
        Some(Snapshot {
            position: playback.duration().min(position.max(0.0)),
            duration: playback.duration(),
            playing: playback.intends_to_play(),
            rate: playback.rate(),
            marks: scrubber_marks(
                &starts,
                playback.duration(),
                detection.as_ref(),
                playback.view_mode() == ViewMode::Representative,
            ),
            message: store.playback_message().map(|message| message.text()),
            older: workspace.adjacent_event(Step::Previous).is_some(),
            newer: workspace.adjacent_event(Step::Next).is_some(),
        })
    }

    fn input(&mut self, input: PlaybackInput, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| store.playback(input, cx));
    }

    fn scrub_to(&mut self, x: Pixels, cx: &mut Context<Self>) {
        let Some(bounds) = self.scrubber.get() else {
            return;
        };
        let duration = self
            .store
            .read(cx)
            .session()
            .map_or(0.0, |session| session.playback.duration());
        let seconds = seconds_at(
            x.as_f32(),
            bounds.origin.x.as_f32(),
            bounds.size.width.as_f32(),
            duration,
        );
        self.input(PlaybackInput::Seek(seconds), cx);
    }

    fn scrubber(&self, snapshot: &Snapshot, cx: &mut Context<Self>) -> AnyElement {
        let palette = AppTheme::get(cx).palette;
        let control_shadow = AppTheme::get(cx).control_shadow.clone();
        let bounds = self.scrubber.clone();
        let entity = cx.entity().downgrade();
        let dragging = self.dragging;
        let progress = fraction(snapshot.position, snapshot.duration);
        // Every mark is placed by its fraction of the rail (the track minus half a thumb each side).
        let at = |fraction: f32| gpui_kit::relative(fraction);
        let rail = div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(RAIL_INSET))
            .right(px(RAIL_INSET));
        let track = div()
            .absolute()
            .left_0()
            .right_0()
            .top(px(TRACK_TOP))
            .h(px(TRACK_HEIGHT))
            .rounded(px(TRACK_HEIGHT / 2.0))
            .bg(palette.fill)
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .h_full()
                    .w(at(progress))
                    .rounded(px(TRACK_HEIGHT / 2.0))
                    .bg(palette.drive),
            )
            .children(snapshot.marks.ticks.iter().map(|tick| {
                div()
                    .absolute()
                    .left(at(*tick))
                    .top(px(-3.0))
                    .ml(px(-1.0))
                    .w(px(2.0))
                    .h(px(12.0))
                    .rounded(px(1.0))
                    .bg(palette.recent_marker)
            }));
        // A box `MARK_BOX` wide whose centre sits at `fraction` of the rail.
        let mark_box = |fraction: f32| {
            div()
                .absolute()
                .left(at(fraction))
                .ml(px(-MARK_BOX / 2.0))
                .w(px(MARK_BOX))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(1.0))
                .ui_text(11.0, 600.0)
                .line_height(relative(1.4))
                .whitespace_nowrap()
        };
        let detection = snapshot.marks.detection.map(|fraction| {
            mark_box(fraction)
                .top(px(-12.0))
                .text_color(palette.sentry_ink)
                .child(copy::DETECTION_TICK)
                .child(
                    div()
                        .w(px(2.0))
                        .h(px(12.0))
                        .rounded(px(1.0))
                        .bg(palette.sentry),
                )
        });
        let pre_roll = snapshot.marks.pre_roll.map(|fraction| {
            mark_box(fraction)
                .top(px(21.0))
                .text_color(palette.ink_3)
                .font_weight(FontWeight::MEDIUM)
                .child(
                    div()
                        .w(px(6.0))
                        .h(px(12.0))
                        .rounded(px(3.0))
                        .border_2()
                        .border_color(palette.ink_3)
                        .bg(palette.surface),
                )
                .child(copy::PRE_ROLL_TICK)
        });
        let thumb = div()
            .absolute()
            .left(at(progress))
            .ml(px(-THUMB / 2.0))
            .top(px(TRACK_TOP + TRACK_HEIGHT / 2.0 - THUMB / 2.0))
            .size(px(THUMB))
            .rounded_full()
            .bg(palette.surface)
            .shadow({
                let mut shadows = vec![gpui_kit::BoxShadow {
                    color: palette.drive,
                    offset: point(px(0.0), px(0.0)),
                    blur_radius: px(0.0),
                    spread_radius: px(2.0),
                    inset: false,
                }];
                shadows.extend(control_shadow);
                shadows
            });
        div()
            .id("scrubber")
            .role(Role::Slider)
            .aria_label(copy::PLAYBACK_POSITION)
            .aria_value(copy::position_value_text(
                snapshot.position,
                snapshot.duration,
            ))
            .aria_numeric_value(snapshot.position)
            .aria_min_numeric_value(0.0)
            .aria_max_numeric_value(snapshot.duration)
            .relative()
            .flex_1()
            .min_w_0()
            .h(px(SCRUBBER_HEIGHT))
            .rounded(px(SCRUBBER_HEIGHT / 2.0))
            .cursor_pointer()
            .keyboard_focus(palette.surface, cx)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.modifiers.modified() {
                    return;
                }
                let Some(snapshot) = this.snapshot(cx) else {
                    return;
                };
                if let Some(target) =
                    slider_key_target(&event.keystroke.key, snapshot.position, snapshot.duration)
                {
                    this.input(PlaybackInput::Seek(target), cx);
                    cx.stop_propagation();
                }
            }))
            .child(
                canvas(
                    move |element_bounds, _, _| bounds.set(Some(element_bounds)),
                    move |_, _, window, _| {
                        if !dragging {
                            return;
                        }
                        // While dragging, follow the pointer anywhere in the window.
                        let moved = entity.clone();
                        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                            if phase == DispatchPhase::Bubble && event.dragging() {
                                moved
                                    .update(cx, |this, cx| this.scrub_to(event.position.x, cx))
                                    .ok();
                            }
                        });
                        let released = entity.clone();
                        window.on_mouse_event(move |_: &MouseUpEvent, phase, _, cx| {
                            if phase == DispatchPhase::Bubble {
                                released
                                    .update(cx, |this, cx| {
                                        this.dragging = false;
                                        cx.notify();
                                    })
                                    .ok();
                            }
                        });
                    },
                )
                .absolute()
                .size_full(),
            )
            .child(
                rail.child(track)
                    .children(pre_roll)
                    .children(detection)
                    .child(thumb),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    this.dragging = true;
                    this.scrub_to(event.position.x, cx);
                    cx.notify();
                }),
            )
            .test_support()
            .into_any_element()
    }

    fn rate_pill(&self, rate: PlaybackRate, cx: &mut Context<Self>) -> AnyElement {
        let palette = AppTheme::get(cx).palette;
        let hit = AppTheme::get(cx).tokens.sizes.hit_target_desktop;
        let fill_hover = palette.fill_hover;
        let pill = div()
            .id("rate")
            .role(Role::Button)
            .aria_label(copy::PLAYBACK_RATE)
            .aria_expanded(self.rate_open)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .h(px(hit))
            .min_w(px(52.0))
            .px(px(12.0))
            .rounded_full()
            .bg(palette.fill)
            .numeral()
            .text_size(px(14.0))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(palette.ink)
            .cursor_pointer()
            .hover(move |style| style.bg(fill_hover))
            .keyboard_focus(palette.surface, cx)
            .child(rate.label())
            .on_click(cx.listener(|this, _, _, cx| {
                this.rate_open = !this.rate_open;
                cx.notify();
            }));
        let menu = self.rate_open.then(|| {
            let entity = cx.entity().downgrade();
            // Above the pill, centred on it.
            deferred(
                div()
                    .absolute()
                    .right(px(-12.0))
                    .bottom(px(hit + 10.0))
                    .child(
                        popover_surface(16.0, cx)
                            .id("rate-menu")
                            .role(Role::Menu)
                            .aria_label(copy::PLAYBACK_RATE)
                            .occlude()
                            .p(px(6.0))
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .on_mouse_down_out(move |_, _, cx| {
                                entity
                                    .update(cx, |this, cx| {
                                        this.rate_open = false;
                                        cx.notify();
                                    })
                                    .ok();
                            })
                            .children(
                                PlaybackRate::ALL
                                    .map(|choice| rate_item(choice, choice == rate, &palette, cx)),
                            )
                            .test_support(),
                    ),
            )
            .with_priority(1)
        });
        div()
            .relative()
            .child(pill.test_support())
            .children(menu)
            .into_any_element()
    }
}

fn rate_item(
    choice: PlaybackRate,
    selected: bool,
    palette: &Palette,
    cx: &mut Context<TransportView>,
) -> AnyElement {
    let fill = palette.fill;
    div()
        .id(("rate", choice.value().to_bits() as usize))
        .role(Role::MenuItemRadio)
        .aria_label(choice.label())
        .aria_toggled(if selected {
            gpui_kit::Toggled::True
        } else {
            gpui_kit::Toggled::False
        })
        .flex()
        .items_center()
        .justify_center()
        .h(px(32.0))
        .w(px(64.0))
        .rounded(px(10.0))
        .numeral()
        .text_size(px(14.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(if selected { palette.drive } else { palette.ink })
        .when(selected, |item| item.bg(palette.drive_soft))
        .when(!selected, |item| item.hover(move |style| style.bg(fill)))
        .cursor_pointer()
        .keyboard_focus(palette.surface, cx)
        .child(choice.label())
        .on_click(cx.listener(move |this, _, _, cx| {
            this.rate_open = false;
            this.input(PlaybackInput::SetRate(choice), cx);
        }))
        .test_support()
        .into_any_element()
}

/// A ±10 s button: the rotate arrow with a small `10` inside.
fn skip_button(forward: bool, cx: &mut Context<TransportView>) -> AnyElement {
    let palette = AppTheme::get(cx).palette;
    let (id, glyph, label, seconds) = if forward {
        ("skip-forward", Icon::SkipForward, copy::SKIP_FORWARD, 10.0)
    } else {
        ("skip-back", Icon::SkipBack, copy::SKIP_BACK, -10.0)
    };
    icon_button(id, glyph, label, true, cx)
        .relative()
        .child(
            div()
                .absolute()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .pt(px(1.0))
                .numeral()
                .text_size(px(8.5))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.ink_2)
                .child(copy::SKIP_NUMERAL),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
            this.input(PlaybackInput::Skip(seconds), cx);
        }))
        .test_support()
        .into_any_element()
}

impl Render for TransportView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(snapshot) = self.snapshot(cx) else {
            return div().into_any_element();
        };
        let theme = AppTheme::get(cx);
        let (palette, tokens) = (theme.palette, theme.tokens);
        let ink = palette.ink;
        let play_size = tokens.sizes.play_button;
        let older = icon_button(
            "older-event",
            Icon::ChevronLeft,
            copy::PREVIOUS_EVENT,
            snapshot.older,
            cx,
        )
        .when(snapshot.older, |button| {
            button.on_click(cx.listener(|this, _, _, cx| {
                this.store.update(cx, |store, cx| {
                    store.dispatch(
                        teslacam_core::app_state::AppAction::EventStepped(Step::Previous),
                        cx,
                    )
                });
            }))
        });
        let newer = icon_button(
            "newer-event",
            Icon::ChevronRight,
            copy::NEXT_EVENT,
            snapshot.newer,
            cx,
        )
        .when(snapshot.newer, |button| {
            button.on_click(cx.listener(|this, _, _, cx| {
                this.store.update(cx, |store, cx| {
                    store.dispatch(
                        teslacam_core::app_state::AppAction::EventStepped(Step::Next),
                        cx,
                    )
                });
            }))
        });
        let play = div()
            .id("play")
            .role(Role::Button)
            .aria_label(if snapshot.playing {
                copy::PAUSE
            } else {
                copy::PLAY
            })
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(play_size))
            .rounded_full()
            .bg(ink)
            .cursor_pointer()
            .keyboard_focus(palette.surface, cx)
            .hover(move |style| style.bg(ink.opacity(0.86)))
            .child(icon(
                if snapshot.playing {
                    Icon::Pause
                } else {
                    Icon::Play
                },
                24.0,
                palette.surface,
            ))
            .on_click(cx.listener(|this, _, _, cx| this.input(PlaybackInput::TogglePlay, cx)));
        let clock = div()
            .flex()
            .flex_none()
            .flex_col()
            .min_w(px(92.0))
            .child(
                div()
                    .type_role(&tokens.type_scale.clock)
                    .text_color(palette.ink)
                    .child(copy::clock_text(snapshot.position)),
            )
            .child(
                div()
                    .mt(px(3.0))
                    .numeral()
                    .text_size(px(13.0))
                    .font_weight(FontWeight::MEDIUM)
                    .line_height(relative(1.0))
                    .text_color(palette.ink_3)
                    .child(copy::clock_duration_text(snapshot.duration)),
            );
        let tools = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(2.0))
            .child(skip_button(false, cx))
            .child(skip_button(true, cx))
            .child(div().w(px(4.0)))
            .child(self.rate_pill(snapshot.rate, cx))
            .child(newer.test_support());
        let row = div()
            .flex()
            .items_center()
            .gap(px(14.0))
            .h(px(86.0))
            .pl(px(10.0))
            .pr(px(16.0))
            .child(older.test_support())
            .child(play.test_support())
            .child(clock)
            .child(self.scrubber(&snapshot, cx))
            .child(tools);
        super::components::glass(tokens.radii.panel, cx)
            .id("transport")
            .role(Role::Group)
            .aria_label(copy::TRANSPORT_REGION)
            .size_full()
            .flex()
            .flex_col()
            .child(row)
            .when_some(snapshot.message, |transport, message| {
                transport.child(
                    div()
                        .id("playback-message")
                        .role(Role::Status)
                        .mt(px(-4.0))
                        .pb(px(14.0))
                        .pl(px(10.0))
                        .pr(px(16.0))
                        .flex()
                        .flex_col()
                        .ui_text(12.0, 400.0)
                        .line_height(relative(1.5))
                        .text_color(palette.warning)
                        .children(message.lines().map(|line| div().child(line)))
                        .test_support(),
                )
            })
            .into_any_element()
    }
}
