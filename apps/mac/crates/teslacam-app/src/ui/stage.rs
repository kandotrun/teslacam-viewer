//! The stage: the car on the garage floor with six camera windows around it and the field-of-view
//! wedges, the 6分割 grid, an expanded camera above a strip of the other five with the mini car,
//! and the representative `event.mp4` (`DESIGN.md` §6 Stage).
//!
//! The stage also runs the frame loop: while the session's media needs it (playing, loading,
//! settling a seek, exporting), every display frame ticks the media through the store and draws
//! the frames the players hand out. Paused and settled, nothing runs.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    Animation, AnimationExt as _, AnyElement, Bounds, Context, Entity, Hsla, Image, ImageFormat,
    InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ObjectFit, ParentElement as _, Pixels, Render, RenderImage, Role,
    StatefulInteractiveElement as _, Styled as _, StyledImage as _, Subscription,
    TestSupportExt as _, Transformation, Window, canvas, div, img, pattern_slash, percentage,
    point, px, surface,
};
use teslacam_core::camera::{CameraId, CameraMap};
use teslacam_core::copy;
use teslacam_core::playback::{PlaybackInput, PlaybackPhase, TileStatus, ViewMode};
use teslacam_media::{VideoFrame, host_now};

use super::components::{KeyboardFocus as _, hairline, ring};
use super::style::{AppTheme, Palette, TypeExt as _, icon};
use crate::assets::{Icon, vehicle_image};
use crate::bridge::{FrameLoop, POLL_INTERVAL};
use crate::dev::DevFlags;
use crate::media::RepresentativeView;
use crate::presentation::floor_art::{CAR_SHADOW_BOX, car_shadow_svg, wedges_svg};
use crate::presentation::floor_layout::floor_layout;
use crate::presentation::motion::{Reveal, Sheet, lerp};
use crate::presentation::stage_layout::{
    Rect, Wedge, compact_chrome, expanded_layout, grid_layout, representative_layout,
    switcher_layout, vehicle_layout, video_rect,
};
use crate::presentation::transport::{fraction, seconds_at};
use crate::store::AppStore;
use crate::sync_probe::{SyncStats, decode_frame};
use crate::ui::floor_phase;

/// What a window shows.
#[derive(Debug, Clone, Copy, PartialEq)]
enum WindowFill {
    /// Before a library: white at 55 % with the camera name centred.
    Empty,
    /// The session's window: video or a state; `reveal` is how far the orchestrated fill has
    /// progressed (1 = done).
    Session { status: TileStatus, reveal: f32 },
}

/// How a window's ring is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RingMode {
    /// The `drive` ring of the active camera.
    Active,
    /// An `ink` ring while hovered.
    Hover,
    /// Never (the expanded camera, empty windows).
    None,
}

/// One camera window to draw.
struct WindowSpec {
    camera: CameraId,
    rect: Rect,
    fill: WindowFill,
    frame: Option<VideoFrame>,
    ring: RingMode,
    expanded: bool,
}

/// The representative video's scrubber while it is dragged.
#[derive(Default)]
struct Scrub {
    bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    dragging: bool,
}

/// The stage view.
pub struct StageView {
    store: Entity<AppStore>,
    car: Arc<Image>,
    car_shadow: Arc<Image>,
    wedge_art: Option<(WedgeKey, Arc<RenderImage>)>,
    tick_scheduled: bool,
    scrub: Scrub,
    sync: Option<(SyncStats, Instant)>,
    /// The camera windows' rects in the last frame, the expanded camera they were drawn for, and
    /// the running `motion-sheet` transition (its start and the rects it starts from).
    drawn: CameraMap<Option<Rect>>,
    drawn_expanded: Option<Option<CameraId>>,
    transition: Option<(Instant, CameraMap<Option<Rect>>)>,
    _observe: Subscription,
}

/// What the rendered wedge image depends on.
#[derive(Debug, Clone, Copy, PartialEq)]
struct WedgeKey {
    width: f32,
    height: f32,
    scale: f32,
    active: CameraId,
    opacity: f32,
}

impl StageView {
    /// A stage reading `store`.
    pub fn new(store: Entity<AppStore>, cx: &mut Context<Self>) -> StageView {
        let palette = &AppTheme::get(cx).tokens.colors;
        let car_shadow = Arc::new(Image::from_bytes(
            ImageFormat::Svg,
            car_shadow_svg(palette.ink).into_bytes(),
        ));
        let sync_probe = cx
            .try_global::<DevFlags>()
            .is_some_and(|flags| flags.sync_probe);
        StageView {
            _observe: cx.observe(&store, |_, _, cx| cx.notify()),
            store,
            car: vehicle_image(),
            car_shadow,
            wedge_art: None,
            tick_scheduled: false,
            scrub: Scrub::default(),
            sync: sync_probe.then(|| (SyncStats::default(), Instant::now())),
            drawn: CameraMap::from_fn(|_| None),
            drawn_expanded: None,
            transition: None,
        }
    }

    /// Runs one step of the frame loop on the next display frame.
    fn schedule_tick(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.tick_scheduled {
            return;
        }
        self.tick_scheduled = true;
        cx.on_next_frame(window, |this, _, cx| {
            this.tick_scheduled = false;
            let now = host_now();
            this.store.update(cx, |store, cx| store.media_tick(now, cx));
            cx.notify();
        });
    }

    /// Runs one step of the frame loop after [`POLL_INTERVAL`] (a camera is still loading while
    /// nothing else moves).
    fn schedule_poll(&mut self, cx: &mut Context<Self>) {
        if self.tick_scheduled {
            return;
        }
        self.tick_scheduled = true;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(POLL_INTERVAL).await;
            let _ = this.update(cx, |this, cx| {
                this.tick_scheduled = false;
                let now = host_now();
                this.store.update(cx, |store, cx| store.media_tick(now, cx));
                cx.notify();
            });
        })
        .detach();
    }

    /// Development: records how far apart the drawn frames' counters are.
    fn probe_sync(&mut self, frames: &CameraMap<Option<VideoFrame>>) {
        let Some((stats, last)) = &mut self.sync else {
            return;
        };
        let stamps: Vec<Option<u32>> = frames
            .iter()
            .filter_map(|(_, frame)| frame.as_ref())
            .map(|frame| decode_frame(&frame.buffer))
            .collect();
        stats.record(&stamps);
        if last.elapsed() >= Duration::from_secs(2) {
            eprintln!("{}", stats.summary());
            *last = Instant::now();
        }
    }
}

fn place(rect: Rect) -> gpui_kit::Div {
    div()
        .absolute()
        .left(px(rect.x))
        .top(px(rect.y))
        .w(px(rect.width))
        .h(px(rect.height))
}

/// The window's name pill (bottom-left over video, centred over an empty window).
fn label(camera: CameraId, empty: bool, compact: bool, palette: &Palette) -> gpui_kit::Div {
    let (height, padding, font, inset) = if compact {
        (20.0, 8.0, 11.0, 6.0)
    } else {
        (24.0, 10.0, 12.0, 10.0)
    };
    let pill = div()
        .absolute()
        .flex()
        .items_center()
        .h(px(height))
        .px(px(padding))
        .rounded_full()
        .ui_text(font, 500.0)
        .whitespace_nowrap()
        .child(camera.label());
    if empty {
        pill.top_0()
            .bottom_0()
            .left_0()
            .right_0()
            .m_auto()
            .justify_center()
            .text_color(palette.ink_2)
    } else {
        pill.left(px(inset))
            .bottom(px(inset))
            .bg(palette.video_label_scrim)
            .text_color(palette.video_text)
    }
}

/// The loading spinner and `読み込み中`, or a fault: stripes, the camera-off icon and the state.
///
/// Every layer is rounded itself: GPUI clips children to rectangles only.
fn state_overlay(
    camera: CameraId,
    status: TileStatus,
    compact: bool,
    radius: f32,
    palette: &Palette,
) -> AnyElement {
    let text = status.label().unwrap_or_default();
    let content = div()
        .absolute()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap(px(6.0))
        .pb(px(if compact { 28.0 } else { 40.0 }))
        .text_color(palette.video_muted)
        .ui_text(12.0, 400.0);
    if status == TileStatus::Loading {
        let spinner = icon(Icon::LoaderCircle, 20.0, palette.video_muted).with_animation(
            ("loading", camera.index()),
            Animation::new(Duration::from_millis(900)).repeat(),
            |spinner, delta| spinner.with_transformation(Transformation::rotate(percentage(delta))),
        );
        return div()
            .absolute()
            .size_full()
            .child(
                content
                    .child(spinner)
                    .when(!compact, |content| content.child(text)),
            )
            .into_any_element();
    }
    div()
        .absolute()
        .size_full()
        .rounded(px(radius))
        .bg(palette.video_missing_a)
        .child(
            div()
                .absolute()
                .size_full()
                .rounded(px(radius))
                .bg(pattern_slash(palette.video_missing_b, 10.0, 10.0)),
        )
        .child(
            content
                .child(icon(Icon::VideoOff, 22.0, palette.video_muted))
                .when(!compact, |content| content.child(text)),
        )
        .into_any_element()
}

/// The 2 pt ring 2 pt outside a window (hover: `ink`, active: `drive`) over a `floor-light` gap.
fn ring_shadow(gap: Hsla, color: Hsla) -> Vec<gpui_kit::BoxShadow> {
    ring(gap, 2.0, color, 2.0)
}

/// A video frame drawn where [`video_rect`] puts it inside a `width` × `height` window.
fn video_surface(frame: VideoFrame, width: f32, height: f32, radius: f32) -> gpui_kit::Surface {
    let rect = video_rect(
        Rect::new(0.0, 0.0, width, height),
        (frame.display_width, frame.display_height),
        radius,
    );
    surface(frame.buffer)
        .absolute()
        .left(px(rect.x))
        .top(px(rect.y))
        .w(px(rect.width))
        .h(px(rect.height))
        .object_fit(ObjectFit::Fill)
}

impl StageView {
    /// The wedge image for `key`, rendered again (and the previous one released) when the stage size,
    /// the active camera or the opacity change.
    fn wedge_image(
        &mut self,
        key: WedgeKey,
        wedges: &CameraMap<Wedge>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        if let Some((current, image)) = &self.wedge_art
            && *current == key
        {
            return Some(image.clone());
        }
        let colors = &AppTheme::get(cx).tokens.colors;
        let svg = wedges_svg(
            key.width,
            key.height,
            wedges,
            Some(key.active),
            (colors.wedge, colors.wedge_active),
            key.opacity,
        );
        let image = cx
            .svg_renderer()
            .render_single_frame(svg.as_bytes(), key.scale)
            .ok()?;
        if let Some((_, previous)) = self.wedge_art.replace((key, image.clone())) {
            cx.drop_image(previous, Some(window));
        }
        Some(image)
    }

    /// A camera window.
    fn camera_window(&self, spec: WindowSpec, cx: &mut Context<Self>) -> AnyElement {
        let theme = AppTheme::get(cx);
        let (palette, radius) = (theme.palette, theme.tokens.radii.camera);
        let WindowSpec {
            camera,
            rect,
            fill,
            frame,
            ring,
            expanded,
        } = spec;
        let compact = compact_chrome(rect);
        let WindowFill::Session { status, reveal } = fill else {
            return place(rect)
                .rounded(px(radius))
                .bg(palette.surface.opacity(0.55))
                .shadow(vec![hairline(palette.ink.opacity(0.06))])
                .child(label(camera, true, compact, &palette))
                .into_any_element();
        };
        let video = frame.filter(|_| status == TileStatus::Ready && reveal >= 1.0);
        let content = div()
            .absolute()
            .size_full()
            .rounded(px(radius))
            .opacity(reveal)
            .bg(palette.video)
            .when_some(video, |content, frame| {
                content.child(video_surface(frame, rect.width, rect.height, radius))
            })
            .when(status != TileStatus::Ready, |content| {
                content.child(state_overlay(camera, status, compact, radius, &palette))
            })
            .child(label(camera, false, compact, &palette));
        let (floor_light, ink) = (palette.floor_light, palette.ink);
        div()
            .id(("camera", camera.index()))
            .role(Role::Button)
            .aria_label(if expanded {
                copy::camera_collapse_label(camera)
            } else {
                copy::camera_expand_label(camera)
            })
            .absolute()
            .left(px(rect.x))
            .top(px(rect.y))
            .w(px(rect.width))
            .h(px(rect.height))
            .rounded(px(radius))
            .cursor_pointer()
            .keyboard_focus(palette.floor, cx)
            .when(reveal < 1.0, |window| {
                window
                    .bg(palette.surface.opacity(0.55))
                    .child(label(camera, true, compact, &palette).opacity(1.0 - reveal))
            })
            .child(content)
            .map(|window| match ring {
                RingMode::Active => window.shadow(ring_shadow(floor_light, palette.drive)),
                RingMode::Hover => {
                    window.hover(move |style| style.shadow(ring_shadow(floor_light, ink)))
                }
                RingMode::None => window,
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.store.update(cx, |store, cx| {
                    store.playback(PlaybackInput::ToggleCamera(camera), cx);
                });
            }))
            .test_support()
            .into_any_element()
    }

    /// The mini car next to the strip: one dot per camera at its place around the car.
    fn mini_car(&self, area: Rect, focus: CameraId, cx: &mut Context<Self>) -> AnyElement {
        let palette = AppTheme::get(cx).palette;
        let layout = switcher_layout(area);
        let (fill, drive_soft) = (palette.fill, palette.drive_soft);
        div()
            .id("camera-switcher")
            .role(Role::Group)
            .aria_label(copy::CAMERA_SWITCHER)
            .absolute()
            .top_0()
            .left_0()
            .child(
                place(layout.car).child(
                    img(self.car.clone())
                        .size_full()
                        .object_fit(ObjectFit::Contain),
                ),
            )
            .children(CameraId::ALL.map(|camera| {
                let pressed = camera == focus;
                let (x, y) = layout.dots[camera];
                let button = layout.buttons[camera];
                div()
                    .id(("show-camera", camera.index()))
                    .role(Role::Button)
                    .aria_label(copy::camera_show_label(camera))
                    .absolute()
                    .left(px(button.x))
                    .top(px(button.y))
                    .size(px(button.width))
                    .rounded_full()
                    .cursor_pointer()
                    .keyboard_focus(palette.floor, cx)
                    .when(pressed, |button| button.bg(drive_soft))
                    .when(!pressed, |button| button.hover(move |style| style.bg(fill)))
                    .child(
                        div()
                            .absolute()
                            .left(px(x - button.x - 5.0))
                            .top(px(y - button.y - 5.0))
                            .size(px(10.0))
                            .rounded_full()
                            .bg(if pressed {
                                palette.drive
                            } else {
                                palette.recent_marker
                            })
                            .shadow(vec![gpui_kit::BoxShadow {
                                color: palette.surface,
                                offset: point(px(0.0), px(0.0)),
                                blur_radius: px(0.0),
                                spread_radius: px(3.0),
                                inset: false,
                            }]),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.store.update(cx, |store, cx| {
                            store.playback(PlaybackInput::ShowCamera(camera), cx);
                        });
                    }))
                    .test_support()
            }))
            .test_support()
            .into_any_element()
    }

    /// `event.mp4` in one large window with its own controls.
    fn representative_window(
        &mut self,
        rect: Rect,
        view: Option<RepresentativeView>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = AppTheme::get(cx);
        let (palette, radius) = (theme.palette, theme.tokens.radii.camera);
        let status = view
            .as_ref()
            .map_or(TileStatus::Loading, |view| view.status);
        let (position, duration, playing) = view.as_ref().map_or((0.0, 0.0, false), |view| {
            (view.position, view.duration.unwrap_or(0.0), view.playing)
        });
        let frame = view.and_then(|view| view.frame);
        let bounds = self.scrub.bounds.clone();
        let progress = fraction(position, duration);
        let controls = div()
            .absolute()
            .left(px(12.0))
            .right(px(12.0))
            .bottom(px(10.0))
            .h(px(36.0))
            .flex()
            .items_center()
            .gap(px(10.0))
            .px(px(4.0))
            .rounded_full()
            .bg(palette.video_label_scrim)
            .child(
                div()
                    .id("representative-play")
                    .role(Role::Button)
                    .aria_label(if playing { copy::PAUSE } else { copy::PLAY })
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(28.0))
                    .rounded_full()
                    .cursor_pointer()
                    .keyboard_focus(palette.video_label_scrim, cx)
                    .child(icon(
                        if playing { Icon::Pause } else { Icon::Play },
                        16.0,
                        palette.video_text,
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(media) = this.store.read(cx).media().cloned() {
                            media.update(cx, |media, _| media.toggle_representative(host_now()));
                        }
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .flex_none()
                    .numeral()
                    .text_size(px(13.0))
                    .text_color(palette.video_text)
                    .child(format!(
                        "{} / {}",
                        copy::clock_text(position),
                        copy::clock_text(duration)
                    )),
            )
            .child(
                div()
                    .id("representative-scrubber")
                    .role(Role::Slider)
                    .aria_label(copy::PLAYBACK_POSITION)
                    .relative()
                    .flex_1()
                    .h(px(28.0))
                    .cursor_pointer()
                    .child(
                        canvas(
                            move |element_bounds, _, _| bounds.set(Some(element_bounds)),
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .size_full(),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(px(10.0))
                            .right(px(10.0))
                            .top(px(12.0))
                            .h(px(4.0))
                            .rounded_full()
                            .bg(palette.video_text.opacity(0.3))
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .h_full()
                                    .w(gpui_kit::relative(progress))
                                    .rounded_full()
                                    .bg(palette.video_text),
                            ),
                    )
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            this.scrub.dragging = true;
                            this.scrub_representative(event.position.x, cx);
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        if this.scrub.dragging && event.dragging() {
                            this.scrub_representative(event.position.x, cx);
                        }
                    }))
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, _, _| this.scrub.dragging = false),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|this, _: &MouseUpEvent, _, _| this.scrub.dragging = false),
                    ),
            );
        place(rect)
            .id("representative")
            .role(Role::Group)
            .aria_label(copy::VIEW_REPRESENTATIVE)
            .rounded(px(radius))
            .overflow_hidden()
            .bg(palette.video)
            .when_some(
                frame.filter(|_| status == TileStatus::Ready),
                |window, frame| window.child(video_surface(frame, rect.width, rect.height, radius)),
            )
            .when(status == TileStatus::Loading, |window| {
                window.child(state_overlay(
                    CameraId::Front,
                    TileStatus::Loading,
                    false,
                    radius,
                    &palette,
                ))
            })
            .when(status.is_fault(), |window| {
                window.child(
                    div()
                        .absolute()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .ui_text(13.0, 500.0)
                        .text_color(palette.video_muted)
                        .child(copy::REPRESENTATIVE_UNAVAILABLE),
                )
            })
            .child(
                div()
                    .absolute()
                    .top(px(12.0))
                    .left(px(12.0))
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .px(px(12.0))
                    .py(px(8.0))
                    .rounded(px(12.0))
                    .bg(palette.video_label_scrim)
                    .text_color(palette.video_text)
                    .ui_text(12.0, 400.0)
                    .child(
                        div()
                            .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                            .child(copy::REPRESENTATIVE_TITLE),
                    )
                    .child(
                        div()
                            .text_color(palette.video_muted)
                            .child(copy::REPRESENTATIVE_NOTE),
                    ),
            )
            .when(status == TileStatus::Ready, |window| window.child(controls))
            .into_any_element()
    }

    fn scrub_representative(&mut self, x: Pixels, cx: &mut Context<Self>) {
        let Some(bounds) = self.scrub.bounds.get() else {
            return;
        };
        let Some(media) = self.store.read(cx).media().cloned() else {
            return;
        };
        let now = host_now();
        media.update(cx, |media, _| {
            let duration = media
                .representative_view(now)
                .and_then(|view| view.duration)
                .unwrap_or(0.0);
            let seconds = seconds_at(
                x.as_f32(),
                bounds.origin.x.as_f32(),
                bounds.size.width.as_f32(),
                duration,
            );
            media.seek_representative(seconds, now);
        });
        cx.notify();
    }
}

impl Render for StageView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (tokens, palette) = {
            let theme = AppTheme::get(cx);
            (theme.tokens, theme.palette)
        };
        let viewport = window.viewport_size();
        let viewport = (viewport.width.as_f32(), viewport.height.as_f32());
        let reveal = Reveal::new(&tokens.motion, cx.reduce_motion());
        let now = host_now();
        let (phase, snapshot, elapsed, media, active_loop) = {
            let store = self.store.read(cx);
            let session = store.session();
            let elapsed = store.loaded_at().map_or(Duration::MAX, |loaded| {
                Instant::now().saturating_duration_since(loaded)
            });
            let snapshot = session.map(|session| {
                let playback = &session.playback;
                (
                    playback.view_mode(),
                    playback.expanded_camera(),
                    playback.active_camera(),
                    CameraMap::from_fn(|camera| playback.status(camera)),
                    playback.phase() == PlaybackPhase::Playing,
                )
            });
            (
                floor_phase(store),
                snapshot,
                elapsed,
                store.media().cloned(),
                store.media_active(cx),
            )
        };
        match active_loop {
            FrameLoop::EveryFrame => self.schedule_tick(window, cx),
            FrameLoop::Poll => self.schedule_poll(cx),
            FrameLoop::Idle => {}
        }
        let floor = floor_layout(viewport.0, viewport.1, phase, tokens);
        let stage = Rect::new(0.0, 0.0, floor.stage.width, floor.stage.height);
        let has_session = snapshot.is_some();
        if has_session && elapsed < reveal.total() {
            window.request_animation_frame();
        }
        let mut frames = CameraMap::from_fn(|_| None);
        let mut representative = None;
        if let Some(media) = &media {
            media.update(cx, |media, _| {
                frames = CameraMap::from_fn(|camera| media.frame(camera, now));
                representative = media.representative_view(now);
            });
        }
        if snapshot.as_ref().is_some_and(|snapshot| snapshot.4) {
            self.probe_sync(&frames);
        }
        let fills = CameraMap::from_fn(|camera| match &snapshot {
            None => WindowFill::Empty,
            Some((_, _, _, statuses, _)) => WindowFill::Session {
                status: statuses[camera],
                reveal: reveal.progress(camera, elapsed),
            },
        });
        let (view, expanded, active) = snapshot
            .as_ref()
            .map_or((ViewMode::Vehicle, None, CameraId::Front), |snapshot| {
                (snapshot.0, snapshot.1, snapshot.2)
            });
        // Expanding or collapsing a camera moves the windows over `motion-sheet` (DESIGN.md §7).
        let sheet = Sheet::new(&tokens.motion, cx.reduce_motion());
        if has_session
            && self
                .drawn_expanded
                .is_some_and(|previous| previous != expanded)
        {
            self.transition = Some((Instant::now(), self.drawn));
        }
        self.drawn_expanded = has_session.then_some(expanded);
        let progress = match &self.transition {
            Some((started, _)) if sheet.running(started.elapsed()) => {
                window.request_animation_frame();
                Some(sheet.progress(started.elapsed()))
            }
            _ => None,
        };
        if progress.is_none() {
            self.transition = None;
        }
        let from = self.transition.as_ref().map(|(_, from)| *from);
        let drawn = std::cell::RefCell::new(CameraMap::from_fn(|_| None));
        let spec = |camera: CameraId,
                    target: Rect,
                    ring: RingMode,
                    frames: &mut CameraMap<Option<VideoFrame>>| {
            let rect = match (progress, from.as_ref().and_then(|from| from[camera])) {
                (Some(t), Some(start)) => Rect::new(
                    lerp(start.x, target.x, t),
                    lerp(start.y, target.y, t),
                    lerp(start.width, target.width, t),
                    lerp(start.height, target.height, t),
                ),
                _ => target,
            };
            drawn.borrow_mut()[camera] = Some(target);
            WindowSpec {
                camera,
                rect,
                fill: fills[camera],
                frame: frames[camera].take(),
                ring: if has_session { ring } else { RingMode::None },
                expanded: expanded == Some(camera),
            }
        };
        let ring_for = |camera: CameraId| {
            if camera == active {
                RingMode::Active
            } else {
                RingMode::Hover
            }
        };
        let root = div().relative().size_full();

        if let Some(focus) = expanded {
            let layout = expanded_layout(stage, focus, viewport.1);
            let mut children =
                vec![self.camera_window(spec(focus, layout.main, RingMode::None, &mut frames), cx)];
            for (camera, rect) in &layout.strip {
                children.push(
                    self.camera_window(spec(*camera, *rect, RingMode::Hover, &mut frames), cx),
                );
            }
            children.push(self.mini_car(layout.switcher, focus, cx));
            self.drawn = drawn.take();
            return root.children(children).into_any_element();
        }
        if view == ViewMode::Grid {
            let windows = grid_layout(stage, &tokens.stage);
            let children: Vec<AnyElement> = CameraId::ALL
                .into_iter()
                .map(|camera| {
                    self.camera_window(
                        spec(camera, windows[camera], ring_for(camera), &mut frames),
                        cx,
                    )
                })
                .collect();
            self.drawn = drawn.take();
            return root.children(children).into_any_element();
        }
        if view == ViewMode::Representative {
            let layout = representative_layout(stage, viewport.1);
            let mut children = vec![self.representative_window(layout.main, representative, cx)];
            for (camera, rect) in &layout.strip {
                children.push(
                    self.camera_window(spec(*camera, *rect, ring_for(*camera), &mut frames), cx),
                );
            }
            self.drawn = drawn.take();
            return root.children(children).into_any_element();
        }

        let layout = vehicle_layout(stage, &tokens.stage);
        let (sx, sy, sw, sh) = CAR_SHADOW_BOX;
        let car = layout.car;
        let shadow = Rect::new(
            car.x + sx * car.width,
            car.y + sy * car.height,
            sw * car.width,
            sh * car.height,
        );
        let mount = layout.mounts[active];
        let wedge_art = self.wedge_image(
            WedgeKey {
                width: stage.width.round(),
                height: stage.height.round(),
                scale: window.scale_factor(),
                active,
                opacity: if has_session { 1.0 } else { 0.5 },
            },
            &layout.wedges,
            window,
            cx,
        );
        let windows: Vec<AnyElement> = CameraId::ALL
            .into_iter()
            .map(|camera| {
                self.camera_window(
                    spec(
                        camera,
                        layout.windows[camera],
                        ring_for(camera),
                        &mut frames,
                    ),
                    cx,
                )
            })
            .collect();
        let element = root
            .when_some(wedge_art, |root, art| {
                root.child(
                    img(art)
                        .absolute()
                        .top_0()
                        .left_0()
                        .w(px(stage.width.round()))
                        .h(px(stage.height.round())),
                )
            })
            .child(
                place(shadow).child(
                    img(self.car_shadow.clone())
                        .size_full()
                        .object_fit(ObjectFit::Fill),
                ),
            )
            .child(
                place(car).child(
                    img(self.car.clone())
                        .size_full()
                        .object_fit(ObjectFit::Contain),
                ),
            )
            .child({
                let (x, y) = mount;
                div()
                    .absolute()
                    .left(px(x - 4.0))
                    .top(px(y - 4.0))
                    .size(px(8.0))
                    .rounded_full()
                    .bg(palette.drive)
                    .shadow(vec![gpui_kit::BoxShadow {
                        color: palette.surface.opacity(0.9),
                        offset: point(px(0.0), px(0.0)),
                        blur_radius: px(0.0),
                        spread_radius: px(3.0),
                        inset: false,
                    }])
            })
            .children(windows)
            .into_any_element();
        self.drawn = drawn.take();
        element
    }
}
