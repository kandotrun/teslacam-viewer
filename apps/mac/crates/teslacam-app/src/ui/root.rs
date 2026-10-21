//! The window's root: the garage floor with the stage, the top row and the floating library placed
//! by [`floor_layout`], and the theater keyboard shortcuts.

use std::sync::Arc;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, Focusable, Image, ImageFormat,
    InteractiveElement as _, IntoElement, KeyDownEvent, ObjectFit, ParentElement as _, Render,
    Styled as _, StyledImage as _, Subscription, Window, div, img, px,
};
use teslacam_core::app_state::{AppAction, Step};
use teslacam_core::playback::PlaybackInput;
use teslacam_core::shortcuts::{
    FocusTarget, Modifiers, ShortcutAction, ShortcutContext, theater_shortcut,
};

use super::floor_phase;
use super::library::LibraryPanel;
use super::stage::StageView;
use super::style::AppTheme;
use super::top_row::TopRow;
use crate::presentation::floor_art::floor_svg;
use crate::presentation::floor_layout::floor_layout;
use crate::presentation::stage_layout::Rect;
use crate::store::AppStore;

/// The root view of the main window.
pub struct RootView {
    store: Entity<AppStore>,
    library: Entity<LibraryPanel>,
    top_row: Entity<TopRow>,
    stage: Entity<StageView>,
    focus: FocusHandle,
    floor: Arc<Image>,
    _observe: Subscription,
}

impl RootView {
    /// The floor for `store`; takes keyboard focus so the shortcuts work at once.
    pub fn new(store: Entity<AppStore>, window: &mut Window, cx: &mut Context<Self>) -> RootView {
        let colors = &AppTheme::get(cx).tokens.colors;
        let floor = Arc::new(Image::from_bytes(
            ImageFormat::Svg,
            floor_svg(colors.floor, colors.floor_light).into_bytes(),
        ));
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        RootView {
            library: cx.new(|cx| LibraryPanel::new(store.clone(), window, cx)),
            top_row: cx.new(|cx| TopRow::new(store.clone(), cx)),
            stage: cx.new(|cx| StageView::new(store.clone(), cx)),
            _observe: cx.observe(&store, |_, _, cx| cx.notify()),
            store,
            focus,
            floor,
        }
    }

    /// The library panel (development hooks open its search, date picker or a cluster).
    pub fn library(&self) -> &Entity<LibraryPanel> {
        &self.library
    }

    /// The top row (development hooks open the details popover).
    pub fn top_row(&self) -> &Entity<TopRow> {
        &self.top_row
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let modifiers = Modifiers {
            command: keystroke.modifiers.platform,
            control: keystroke.modifiers.control,
            alt: keystroke.modifiers.alt,
            shift: keystroke.modifiers.shift,
        };
        let search = self.library.read(cx).search_focus(cx);
        let focus = if search.is_focused(window) {
            FocusTarget::TextField
        } else {
            FocusTarget::None
        };
        // Esc closes an open popover before it collapses a camera.
        if keystroke.key == "escape" && focus == FocusTarget::None {
            let closed_details = self.top_row.update(cx, |top, cx| top.close_popovers(cx));
            let closed_picker = self
                .library
                .update(cx, |library, cx| library.close_popovers(cx));
            if closed_details || closed_picker {
                cx.stop_propagation();
                return;
            }
        }
        let Some(context) = self
            .store
            .read(cx)
            .session()
            .map(|session| ShortcutContext {
                focus,
                expanded_camera: session.playback.expanded_camera(),
                dialog_open: session.export.dialog_open(),
            })
        else {
            return;
        };
        let Some(action) = theater_shortcut(&keystroke.key, modifiers, context) else {
            return;
        };
        cx.stop_propagation();
        self.store.update(cx, |store, cx| match action {
            ShortcutAction::TogglePlay => store.playback(PlaybackInput::TogglePlay, cx),
            ShortcutAction::Skip(seconds) => store.playback(PlaybackInput::Skip(seconds), cx),
            ShortcutAction::ToggleCamera(camera) => {
                store.playback(PlaybackInput::ToggleCamera(camera), cx)
            }
            ShortcutAction::Collapse => store.playback(PlaybackInput::Collapse, cx),
            ShortcutAction::PreviousEvent => {
                store.dispatch(AppAction::EventStepped(Step::Previous), cx)
            }
            ShortcutAction::NextEvent => store.dispatch(AppAction::EventStepped(Step::Next), cx),
        });
    }
}

impl Focusable for RootView {
    fn focus_handle(&self, _: &gpui_kit::App) -> FocusHandle {
        self.focus.clone()
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

impl Render for RootView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = AppTheme::get(cx);
        let viewport = window.viewport_size();
        let floor = floor_layout(
            viewport.width.as_f32(),
            viewport.height.as_f32(),
            floor_phase(self.store.read(cx)),
            theme.tokens,
        );
        div()
            .id("floor")
            .track_focus(&self.focus)
            .key_context("Floor")
            .on_key_down(cx.listener(Self::on_key_down))
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(theme.palette.floor)
            .child(
                img(self.floor.clone())
                    .absolute()
                    .size_full()
                    .object_fit(ObjectFit::Fill),
            )
            .child(place(floor.stage).child(self.stage.clone()))
            .when_some(floor.top_row, |root, rect| {
                root.child(place(rect).child(self.top_row.clone()))
            })
            .child(place(floor.library).child(self.library.clone()))
    }
}
