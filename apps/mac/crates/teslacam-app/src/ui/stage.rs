//! The stage: the car on the garage floor with six camera windows around it, the field-of-view
//! wedges, and the 6分割 grid (`DESIGN.md` §6 Stage).
//!
//! Stage 1 draws the windows without video: empty white windows before a library is loaded, then
//! dark video windows (the orchestrated reveal fills them in mount order), with the missing-footage
//! state where the timeline has no clip. Stage 2 puts `teslacam-media` frames into
//! [`video_slot`].

use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AnyElement, Context, Entity, Image, ImageFormat, InteractiveElement as _, IntoElement,
    ObjectFit, ParentElement as _, Render, RenderImage, Role, StatefulInteractiveElement as _,
    Styled as _, StyledImage as _, Subscription, Window, div, img, pattern_slash, point, px,
};
use teslacam_core::camera::{CameraId, CameraMap};
use teslacam_core::copy;
use teslacam_core::playback::{PlaybackInput, TileStatus, ViewMode};

use super::components::{hairline, ring};
use super::style::{AppTheme, Palette, TypeExt as _, icon};
use crate::assets::{Icon, vehicle_image};
use crate::presentation::floor_art::{CAR_SHADOW_BOX, car_shadow_svg, wedges_svg};
use crate::presentation::floor_layout::floor_layout;
use crate::presentation::motion::Reveal;
use crate::presentation::stage_layout::{Rect, Wedge, grid_layout, vehicle_layout};
use crate::store::AppStore;
use crate::ui::floor_phase;

/// What a window shows in stage 1.
#[derive(Debug, Clone, Copy, PartialEq)]
enum WindowFill {
    /// Before a library: white at 55 % with the camera name centred.
    Empty,
    /// The session's window: video (a dark placeholder until stage 2) or a fault state; `reveal`
    /// is how far the orchestrated fill has progressed (1 = done).
    Session { status: TileStatus, reveal: f32 },
}

/// The stage view.
pub struct StageView {
    store: Entity<AppStore>,
    car: Arc<Image>,
    car_shadow: Arc<Image>,
    wedge_art: Option<(WedgeKey, Arc<RenderImage>)>,
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
        StageView {
            _observe: cx.observe(&store, |_, _, cx| cx.notify()),
            store,
            car: vehicle_image(),
            car_shadow,
            wedge_art: None,
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

fn status_text(status: TileStatus) -> Option<&'static str> {
    status.label()
}

/// The window's label pill (bottom-left over video, centred over an empty window).
fn label(camera: CameraId, empty: bool, palette: &Palette) -> gpui_kit::Div {
    let pill = div()
        .absolute()
        .flex()
        .items_center()
        .h(px(24.0))
        .px(px(10.0))
        .rounded_full()
        .ui_text(12.0, 500.0)
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
        pill.left(px(10.0))
            .bottom(px(10.0))
            .bg(palette.video_label_scrim)
            .text_color(palette.video_text)
    }
}

/// Where stage 2 draws the camera's frames (`surface()` of the media layer's pixel buffer).
fn video_slot(palette: &Palette) -> gpui_kit::Div {
    div().absolute().size_full().bg(palette.video)
}

fn fault_overlay(status: TileStatus, palette: &Palette) -> gpui_kit::Div {
    let text = status_text(status).unwrap_or_default();
    div()
        .absolute()
        .size_full()
        .bg(palette.video_missing_a)
        .child(
            div()
                .absolute()
                .size_full()
                .bg(pattern_slash(palette.video_missing_b, 10.0, 10.0)),
        )
        .child(
            div()
                .absolute()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(6.0))
                .pb(px(28.0))
                .text_color(palette.video_muted)
                .ui_text(12.0, 400.0)
                .child(icon(Icon::VideoOff, 22.0, palette.video_muted))
                .child(text),
        )
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

    #[allow(clippy::too_many_arguments)]
    fn camera_window(
        &self,
        camera: CameraId,
        rect: Rect,
        fill: WindowFill,
        active: bool,
        palette: &Palette,
        radius: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let window = place(rect).rounded(px(radius)).overflow_hidden();
        match fill {
            WindowFill::Empty => window
                .bg(palette.surface.opacity(0.55))
                .shadow(vec![hairline(palette.ink.opacity(0.06))])
                .child(label(camera, true, palette))
                .into_any_element(),
            WindowFill::Session { status, reveal } => {
                let content = div()
                    .absolute()
                    .size_full()
                    .opacity(reveal)
                    .child(video_slot(palette))
                    .when(status.is_terminal(), |content| {
                        content.child(fault_overlay(status, palette))
                    })
                    .child(label(camera, false, palette));
                let (floor_light, ink) = (palette.floor_light, palette.ink);
                let frame = div()
                    .id(("camera", camera.index()))
                    .role(Role::Button)
                    .aria_label(copy::camera_expand_label(camera))
                    .absolute()
                    .left(px(rect.x))
                    .top(px(rect.y))
                    .w(px(rect.width))
                    .h(px(rect.height))
                    .rounded(px(radius))
                    .cursor_pointer()
                    .when(reveal < 1.0, |frame| {
                        frame
                            .bg(palette.surface.opacity(0.55))
                            .child(label(camera, true, palette).opacity(1.0 - reveal))
                    })
                    .child(
                        div()
                            .absolute()
                            .size_full()
                            .rounded(px(radius))
                            .overflow_hidden()
                            .child(content),
                    )
                    .map(|frame| {
                        if active {
                            frame.shadow(ring(palette.floor_light, 2.0, palette.drive, 2.0))
                        } else {
                            frame.hover(move |style| style.shadow(ring(floor_light, 2.0, ink, 2.0)))
                        }
                    })
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.store.update(cx, |store, cx| {
                            store.playback(PlaybackInput::ToggleCamera(camera), cx);
                        });
                    }));
                frame.into_any_element()
            }
        }
    }
}

impl Render for StageView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (tokens, palette) = {
            let theme = AppTheme::get(cx);
            (theme.tokens, theme.palette)
        };
        let radius = tokens.radii.camera;
        let viewport = window.viewport_size();
        let reveal = Reveal::new(&tokens.motion, cx.reduce_motion());
        let (phase, view, fills, active, has_session, elapsed) = {
            let store = self.store.read(cx);
            let session = store.session();
            let elapsed = store.loaded_at().map_or(Duration::MAX, |loaded| {
                Instant::now().saturating_duration_since(loaded)
            });
            let fills = CameraMap::from_fn(|camera| match session {
                None => WindowFill::Empty,
                Some(session) => WindowFill::Session {
                    status: session.playback.status(camera),
                    reveal: reveal.progress(camera, elapsed),
                },
            });
            let active =
                session.map_or(CameraId::Front, |session| session.playback.active_camera());
            (
                floor_phase(store),
                session.map(|session| session.playback.view_mode()),
                fills,
                active,
                session.is_some(),
                elapsed,
            )
        };
        let floor = floor_layout(
            viewport.width.as_f32(),
            viewport.height.as_f32(),
            phase,
            tokens,
        );
        let stage = Rect::new(0.0, 0.0, floor.stage.width, floor.stage.height);
        if has_session && elapsed < reveal.total() {
            window.request_animation_frame();
        }
        let root = div().relative().size_full();
        if view == Some(ViewMode::Grid) {
            let windows = grid_layout(stage, &tokens.stage);
            return root
                .children(CameraId::ALL.map(|camera| {
                    self.camera_window(
                        camera,
                        windows[camera],
                        fills[camera],
                        active == camera,
                        &palette,
                        radius,
                        cx,
                    )
                }))
                .into_any_element();
        }
        if view == Some(ViewMode::Representative) {
            // Stage 2: the representative player (`event.mp4`) with native-like controls.
            let inner = stage.inset(4.0, 24.0);
            return root
                .child(
                    place(inner)
                        .rounded(px(radius))
                        .overflow_hidden()
                        .bg(palette.video)
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
                                .child(copy::REPRESENTATIVE_TITLE)
                                .child(
                                    div()
                                        .text_color(palette.video_muted)
                                        .child(copy::REPRESENTATIVE_NOTE),
                                ),
                        ),
                )
                .into_any_element();
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
        root.when_some(wedge_art, |root, art| {
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
        .children(CameraId::ALL.map(|camera| {
            self.camera_window(
                camera,
                layout.windows[camera],
                fills[camera],
                has_session && active == camera,
                &palette,
                radius,
                cx,
            )
        }))
        .into_any_element()
    }
}
