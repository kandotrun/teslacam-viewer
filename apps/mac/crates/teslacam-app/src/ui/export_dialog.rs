//! The composite export dialog (`DESIGN.md` §6 Composite export dialog, Mac form): the event, its
//! full range, a thumbnail of the 1280 × 720 export frame, the resolution and the format, then
//! `書き出しを開始`, which asks for the destination with the save panel.
//!
//! The thumbnail is the export's own overlay (`teslacam_media::overlay::render_preview`: floor,
//! wedges, car and labels with holes where the videos go) over the video colour, drawn once per
//! session on a background thread.

use std::sync::Arc;

use gpui_kit::base::FocusTrapElement as _;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, Image, ImageFormat, InteractiveElement as _,
    IntoElement, ObjectFit, ParentElement as _, Render, Role, StatefulInteractiveElement as _,
    Styled as _, StyledImage as _, Subscription, Task, TestSupportExt as _, Window, div, img, px,
    relative,
};
use teslacam_core::camera::CameraMap;
use teslacam_core::copy;
use teslacam_core::export_state::ExportAction;
use teslacam_core::filters::event_time;
use teslacam_media::layout::ExportLayout;
use teslacam_media::overlay::{VEHICLE_PNG, render_preview};

use super::components::{glass, hairline, icon_button, primary_button, text_button};
use super::style::{AppTheme, TypeExt as _, numeral_text};
use crate::assets::Icon;
use crate::presentation::bitmap::opaque_bmp;
use crate::presentation::floor_layout::floor_layout;
use crate::store::{AppStore, StoreEvent};
use crate::ui::floor_phase;

/// Maximum dialog width.
const DIALOG_WIDTH: f32 = 520.0;

/// The export dialog view (rendered by the root while the export machine's dialog is open).
pub struct ExportDialog {
    store: Entity<AppStore>,
    thumbnail: Option<Arc<Image>>,
    drawing: Option<Task<()>>,
    /// The dialog's focus trap: Tab cycles inside it while it is open.
    focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl ExportDialog {
    /// A dialog reading `store`.
    pub fn new(store: Entity<AppStore>, cx: &mut Context<Self>) -> ExportDialog {
        ExportDialog {
            _subscriptions: vec![
                cx.observe(&store, |_, _, cx| cx.notify()),
                cx.subscribe(&store, |this, _, event, cx| {
                    if *event == StoreEvent::SessionChanged {
                        this.thumbnail = None;
                        this.drawing = None;
                        cx.notify();
                    }
                }),
            ],
            store,
            thumbnail: None,
            drawing: None,
            focus: cx.focus_handle(),
        }
    }

    /// The dialog's focus-trap handle.
    pub fn focus_handle(&self) -> FocusHandle {
        self.focus.clone()
    }

    #[cfg(test)]
    /// Whether keyboard focus is inside the dialog.
    pub fn contains_focus(&self, window: &Window, cx: &gpui_kit::App) -> bool {
        self.focus.contains_focused(window, cx)
    }

    /// Draws the thumbnail on a background thread, with the probed video sizes when known.
    fn draw_thumbnail(&mut self, cx: &mut Context<Self>) {
        if self.thumbnail.is_some() || self.drawing.is_some() {
            return;
        }
        let sizes = self
            .store
            .read(cx)
            .media()
            .map(|media| {
                let media = media.read(cx);
                CameraMap::from_fn(|camera| media.display_size(camera))
            })
            .unwrap_or_else(|| CameraMap::from_fn(|_| None));
        // A camera without footage in the whole event shows `記録なし`, as in the exported video.
        let missing = self
            .store
            .read(cx)
            .session()
            .map(|session| {
                CameraMap::from_fn(|camera| session.timeline.placements(camera).is_empty())
            })
            .unwrap_or_else(|| CameraMap::from_fn(|_| false));
        let video = AppTheme::get(cx).tokens.colors.video;
        let background = cx.background_spawn(async move {
            let layout = ExportLayout::contract();
            let holes = CameraMap::from_fn(|camera| layout.video_rect(camera, sizes[camera]));
            let bitmap = render_preview(&layout, &holes, &missing, VEHICLE_PNG).ok()?;
            Some(opaque_bmp(
                bitmap.width,
                bitmap.height,
                &bitmap.rgba,
                [video.r, video.g, video.b],
            ))
        });
        self.drawing = Some(cx.spawn(async move |this, cx| {
            let bmp = background.await;
            this.update(cx, |this, cx| {
                this.thumbnail = bmp.map(|bmp| Arc::new(Image::from_bytes(ImageFormat::Bmp, bmp)));
                cx.notify();
            })
            .ok();
        }));
    }

    fn dispatch(&mut self, action: ExportAction, cx: &mut Context<Self>) {
        self.store.update(cx, |store, cx| store.export(action, cx));
    }
}

impl Render for ExportDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snapshot = {
            let store = self.store.read(cx);
            store
                .session()
                .zip(store.selected_event())
                .map(|(session, event)| {
                    (
                        copy::export_record_text(&event_time(&event.timestamp), event.category),
                        copy::export_range_text(session.timeline.duration() as u64),
                        session.export.dialog_open(),
                    )
                })
        };
        let Some((record, range, _)) = snapshot.filter(|(_, _, open)| *open) else {
            return div().into_any_element();
        };
        self.draw_thumbnail(cx);
        let theme = AppTheme::get(cx);
        let (palette, tokens) = (theme.palette, theme.tokens);
        let viewport = window.viewport_size();
        let floor = floor_layout(
            viewport.width.as_f32(),
            viewport.height.as_f32(),
            floor_phase(self.store.read(cx)),
            tokens,
        );
        let width = DIALOG_WIDTH.min(floor.stage.width - 32.0).max(320.0);
        let fact = |label: &'static str, value: gpui_kit::AnyElement| {
            div()
                .flex()
                .gap(px(12.0))
                .ui_text(13.0, 500.0)
                .line_height(relative(1.4))
                .child(
                    div()
                        .flex_none()
                        .w(px(72.0))
                        .text_color(palette.ink_3)
                        .child(label),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_color(palette.ink)
                        .child(value),
                )
        };
        let preview = div()
            .mt(px(14.0))
            .w_full()
            .h(px((width - 44.0) * 9.0 / 16.0))
            .rounded(px(12.0))
            .overflow_hidden()
            .bg(palette.floor)
            .shadow(vec![hairline(palette.line)])
            .when_some(self.thumbnail.clone(), |preview, image| {
                preview.child(
                    img(image)
                        .size_full()
                        .rounded(px(12.0))
                        .object_fit(ObjectFit::Contain),
                )
            });
        let dialog = glass(tokens.radii.dialog, cx)
            .id("export-dialog")
            .role(Role::Dialog)
            .aria_label(copy::EXPORT_DIALOG_TITLE)
            .aria_description(copy::EXPORT_DIALOG_DESCRIPTION)
            .occlude()
            .w(px(width))
            .bg(palette.surface)
            .pt(px(20.0))
            .px(px(22.0))
            .pb(px(18.0))
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .mt(px(-6.0))
                    .mr(px(-8.0))
                    .mb(px(8.0))
                    .child(
                        div()
                            .ui_text(18.0, 600.0)
                            .text_color(palette.ink)
                            .child(copy::EXPORT_DIALOG_TITLE),
                    )
                    .child(
                        icon_button(
                            "export-dialog-close",
                            Icon::X,
                            copy::EXPORT_DIALOG_CLOSE,
                            true,
                            cx,
                        )
                        .on_click(
                            cx.listener(|this, _, _, cx| this.dispatch(ExportAction::Dismiss, cx)),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .ui_text(14.0, 400.0)
                    .line_height(relative(1.7))
                    .text_color(palette.ink_2)
                    .children(
                        copy::EXPORT_DIALOG_DESCRIPTION
                            .split_inclusive('。')
                            .map(|sentence| div().child(sentence.to_owned())),
                    ),
            )
            .child(preview)
            .child(
                div()
                    .mt(px(14.0))
                    .flex()
                    .flex_col()
                    .gap(px(6.0))
                    .child(fact(
                        copy::EXPORT_RECORD,
                        numeral_text(&record, 14.0, 500.0, None).into_any_element(),
                    ))
                    .child(fact(
                        copy::EXPORT_RANGE,
                        numeral_text(&range, 14.0, 500.0, None).into_any_element(),
                    ))
                    .child(fact(
                        copy::EXPORT_RESOLUTION,
                        numeral_text(copy::EXPORT_RESOLUTION_VALUE, 14.0, 500.0, None)
                            .into_any_element(),
                    ))
                    .child(fact(
                        copy::EXPORT_FORMAT,
                        div().child(copy::EXPORT_FORMAT_VALUE).into_any_element(),
                    )),
            )
            .child(
                div()
                    .mt(px(18.0))
                    .flex()
                    .justify_end()
                    .items_center()
                    .gap(px(8.0))
                    .child(
                        text_button("export-cancel", copy::EXPORT_CANCEL, cx)
                            .h(px(44.0))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.dispatch(ExportAction::Dismiss, cx)
                            }))
                            .test_support(),
                    )
                    .child(
                        primary_button(
                            "export-start",
                            Icon::Download,
                            copy::EXPORT_START,
                            {
                                let store = self.store.clone();
                                move |_, _, cx| {
                                    store.update(cx, |store, cx| {
                                        store.export(ExportAction::Start, cx)
                                    });
                                }
                            },
                            cx,
                        )
                        .h(px(44.0))
                        .px(px(22.0))
                        .test_support(),
                    ),
            );
        // The scrim covers the window; the dialog is centred on the stage.
        div()
            .id("export-scrim")
            .absolute()
            .size_full()
            .occlude()
            .bg(palette.scrim)
            .child(
                div()
                    .absolute()
                    .left(px(floor.stage.x))
                    .top_0()
                    .w(px(floor.stage.width))
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(dialog.test_support())
                    .focus_trap("export-dialog-trap", &self.focus),
            )
            .into_any_element()
    }
}
