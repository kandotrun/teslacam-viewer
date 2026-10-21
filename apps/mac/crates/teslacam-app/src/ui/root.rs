//! The window's root: the garage floor with the stage, the top row and the floating library placed
//! by [`floor_layout`], and the theater keyboard shortcuts.

use std::sync::Arc;

use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::{
    AppContext as _, Context, Entity, FocusHandle, Focusable, Image, ImageFormat,
    InteractiveElement as _, IntoElement, KeyDownEvent, ObjectFit, ParentElement as _, Render,
    StyleRefinement, Styled as _, StyledImage as _, Subscription, TestSupportExt as _, Window, div,
    img, px,
};
use teslacam_core::app_state::{AppAction, Step};
use teslacam_core::export_state::ExportAction;
use teslacam_core::playback::PlaybackInput;
use teslacam_core::shortcuts::{
    FocusTarget, Modifiers, ShortcutAction, ShortcutContext, theater_shortcut,
};

use super::export_dialog::ExportDialog;
use super::floor_phase;
use super::library::LibraryPanel;
use super::stage::StageView;
use super::style::AppTheme;
use super::top_row::TopRow;
use super::transport::TransportView;
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
    transport: Entity<TransportView>,
    export_dialog: Entity<ExportDialog>,
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
            transport: cx.new(|cx| TransportView::new(store.clone(), cx)),
            export_dialog: cx.new(|cx| ExportDialog::new(store.clone(), cx)),
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

    /// The transport (development hooks open the rate menu).
    pub fn transport(&self) -> &Entity<TransportView> {
        &self.transport
    }

    /// Whether a popover or menu is open (it keeps the theater keys, like a focused popup).
    fn popover_open(&self, cx: &gpui_kit::App) -> bool {
        self.top_row.read(cx).has_open_popover()
            || self.library.read(cx).has_open_popover()
            || self.transport.read(cx).has_open_popover()
    }

    /// What holds keyboard focus for the shortcuts (spec §2.10.10 rule 2): the search field or an
    /// open popover keeps every key; a focused control (any element but the floor itself, which a
    /// click on bare floor focuses) keeps every key but `Esc`.
    fn focus_target(&self, window: &Window, cx: &gpui_kit::App) -> FocusTarget {
        let search = self.library.read(cx).search_focus(cx);
        if search.is_focused(window) || self.popover_open(cx) {
            FocusTarget::TextField
        } else if window
            .focused(cx)
            .is_some_and(|focused| focused != self.focus)
        {
            FocusTarget::Button
        } else {
            FocusTarget::None
        }
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;
        let modifiers = Modifiers {
            command: keystroke.modifiers.platform,
            control: keystroke.modifiers.control,
            alt: keystroke.modifiers.alt,
            shift: keystroke.modifiers.shift,
        };
        if keystroke.key == "tab" && !modifiers.command && !modifiers.control && !modifiers.alt {
            if modifiers.shift {
                window.focus_prev(cx);
            } else {
                window.focus_next(cx);
            }
            cx.stop_propagation();
            return;
        }
        let focus = self.focus_target(window, cx);
        // Esc cancels the export dialog, then closes an open popover, then collapses a camera.
        let dialog_open = self
            .store
            .read(cx)
            .session()
            .is_some_and(|session| session.export.dialog_open());
        if keystroke.key == "escape" && dialog_open {
            self.store
                .update(cx, |store, cx| store.export(ExportAction::Dismiss, cx));
            cx.stop_propagation();
            return;
        }
        let in_search = self.library.read(cx).search_focus(cx).is_focused(window);
        if keystroke.key == "escape" && !in_search {
            let closed_details = self.top_row.update(cx, |top, cx| top.close_popovers(cx));
            let closed_picker = self
                .library
                .update(cx, |library, cx| library.close_popovers(cx));
            let closed_rate = self
                .transport
                .update(cx, |transport, cx| transport.close_popovers(cx));
            if closed_details || closed_picker || closed_rate {
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
        let dialog_open = self
            .store
            .read(cx)
            .session()
            .is_some_and(|session| session.export.dialog_open());
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
            .when_some(floor.transport, |root, rect| {
                root.child(place(rect).child(self.transport.clone()))
            })
            .when_some(floor.top_row, |root, rect| {
                root.child(
                    place(rect).child(
                        self.top_row
                            .clone()
                            .cached(StyleRefinement::default().size_full()),
                    ),
                )
            })
            .child(
                place(floor.library).child(
                    self.library
                        .clone()
                        .cached(StyleRefinement::default().size_full()),
                ),
            )
            .when(dialog_open, |root| root.child(self.export_dialog.clone()))
            .test_support()
    }
}
