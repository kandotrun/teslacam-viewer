//! TeslaCam Viewer for macOS.
//!
//! Startup: GPUI Kit with the app's icons, Barlow, the design tokens, the light (Aqua) appearance,
//! the menu bar (フォルダを開く… ⌘O, 隠す ⌘H, 終了 ⌘Q), and one 1440 × 900 window (minimum
//! 1100 × 700) whose root is the garage floor. Development affordances are described in [`dev`].

mod assets;
mod dev;
mod presentation;
mod scan;
mod session;
mod store;
mod ui;

use std::time::Duration;

use gpui_kit::{
    AnyWindowHandle, App, AppContext as _, AsyncApp, Bounds, Entity, KeyBinding, Menu, MenuItem,
    TitlebarOptions, WindowBounds, WindowOptions, px, size,
};
use objc2::MainThreadMarker;
use objc2_app_kit::{NSAppearance, NSAppearanceNameAqua, NSApplication, NSView};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use teslacam_core::app_state::AppAction;
use teslacam_core::copy;
use teslacam_core::filters::CategoryFilter;
use teslacam_core::playback::{PlaybackInput, ViewMode};

use crate::dev::{DevOptions, DevUi, DevView};
use crate::store::{AppStore, StoreEvent};
use crate::ui::root::RootView;
use crate::ui::style::AppTheme;

gpui_kit::actions!(teslacam, [OpenFolder, Hide, Quit]);

/// Default and minimum window content sizes (spec header decision).
const DEFAULT_SIZE: (f32, f32) = (1440.0, 900.0);
const MINIMUM_SIZE: (f32, f32) = (1100.0, 700.0);

/// The web forces a light colour scheme; the Mac app renders Aqua even in Dark Mode (spec §2.1).
fn force_light_appearance() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    // SAFETY: `NSAppearanceNameAqua` is an immutable AppKit constant.
    let aqua = NSAppearance::appearanceNamed(unsafe { NSAppearanceNameAqua });
    NSApplication::sharedApplication(mtm).setAppearance(aqua.as_deref());
}

fn install_menus(store: &Entity<AppStore>, cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-o", OpenFolder, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("cmd-q", Quit, None),
    ]);
    let store = store.clone();
    cx.on_action(move |_: &OpenFolder, cx| {
        store.update(cx, |store, cx| store.choose_folder(cx));
    });
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.set_menus([
        Menu {
            name: copy::APP_NAME.into(),
            items: vec![
                MenuItem::action(copy::MENU_HIDE, Hide),
                MenuItem::separator(),
                MenuItem::action(copy::MENU_QUIT, Quit),
            ],
            disabled: false,
        },
        Menu {
            name: copy::MENU_FILE.into(),
            items: vec![MenuItem::action(copy::MENU_OPEN_FOLDER, OpenFolder)],
            disabled: false,
        },
    ]);
}

/// The window's `CGWindowID` (for `screencapture -l`).
fn window_number(window: &gpui_kit::Window) -> Option<isize> {
    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return None;
    };
    // SAFETY: GPUI's AppKit handle points at the window's live content view; we only read its
    // window number on the main thread.
    let view: &NSView = unsafe { appkit.ns_view.cast::<NSView>().as_ref() };
    view.window().map(|window| window.windowNumber())
}

/// Applies the development options that need a loaded library.
fn apply_library_options(
    options: &DevOptions,
    store: &Entity<AppStore>,
    window: AnyWindowHandle,
    root: &Entity<RootView>,
    cx: &mut App,
) {
    for item in &options.ui {
        if let DevUi::Category(category) = item {
            store.update(cx, |store, cx| {
                store.dispatch(
                    AppAction::CategoryFiltered(CategoryFilter::Only(*category)),
                    cx,
                );
            });
        }
    }
    if let Some(event_id) = &options.select {
        store.update(cx, |store, cx| {
            store.dispatch(AppAction::EventSelected(event_id.clone()), cx);
        });
    }
    store.update(cx, |store, cx| {
        match options.view {
            Some(DevView::Vehicle) => {
                store.playback(PlaybackInput::SetViewMode(ViewMode::Vehicle), cx)
            }
            Some(DevView::Grid) => store.playback(PlaybackInput::SetViewMode(ViewMode::Grid), cx),
            Some(DevView::Representative) => {
                store.playback(PlaybackInput::SetViewMode(ViewMode::Representative), cx)
            }
            Some(DevView::Expanded(camera)) => {
                store.playback(PlaybackInput::ToggleCamera(camera), cx)
            }
            None => {}
        }
        if let Some(seconds) = options.seek {
            store.playback(PlaybackInput::Seek(seconds), cx);
        }
        if options.play {
            store.playback(PlaybackInput::Play, cx);
        }
    });
    let ui = options.ui.clone();
    let root = root.clone();
    cx.spawn(async move |cx: &mut AsyncApp| {
        // Let the rail measure itself before clusters are looked up.
        cx.background_executor()
            .timer(Duration::from_millis(300))
            .await;
        window
            .update(cx, |_, window, cx| {
                let view = root.read(cx);
                let (top_row, library) = (view.top_row().clone(), view.library().clone());
                for item in &ui {
                    match item {
                        DevUi::Details => top_row.update(cx, |top, cx| top.open_details(false, cx)),
                        DevUi::Technical => {
                            top_row.update(cx, |top, cx| top.open_details(true, cx))
                        }
                        DevUi::DatePicker => {
                            library.update(cx, |library, cx| library.open_date_picker(cx))
                        }
                        DevUi::Search(query) => {
                            library.update(cx, |library, cx| library.open_search(query, window, cx))
                        }
                        DevUi::Cluster(event_id) => {
                            library.update(cx, |library, cx| library.open_cluster_of(event_id, cx))
                        }
                        DevUi::Category(_) => {}
                    }
                }
            })
            .ok();
    })
    .detach();
}

fn schedule_capture(options: &DevOptions, window: AnyWindowHandle, cx: &mut App) {
    let Some(path) = options.capture.clone() else {
        return;
    };
    let delay = options.capture_delay;
    cx.spawn(async move |cx: &mut AsyncApp| {
        cx.background_executor().timer(delay).await;
        let number = window
            .update(cx, |_, window, _| window_number(window))
            .ok()
            .flatten();
        match number {
            Some(number) => {
                let status = std::process::Command::new("screencapture")
                    .args(["-x", "-o", "-l", &number.to_string()])
                    .arg(&path)
                    .status();
                eprintln!("capture {} -> {status:?}", path.display());
            }
            None => eprintln!("capture: no window number"),
        }
        cx.update(|cx| cx.quit());
    })
    .detach();
}

fn main() {
    let options = DevOptions::from_process();
    gpui_kit::application()
        .with_assets(assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            gpui_kit::component::Theme::change(gpui_kit::component::ThemeMode::Light, None, cx);
            assets::register_fonts(cx);
            AppTheme::install(cx);
            force_light_appearance();

            let store = cx.new(|_| AppStore::new(options.scan_delay));
            install_menus(&store, cx);

            let (width, height) = options.window_size.unwrap_or(DEFAULT_SIZE);
            let bounds = Bounds::centered(None, size(px(width), px(height)), cx);
            let (window, root) = gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some(copy::APP_NAME.into()),
                        ..Default::default()
                    }),
                    window_min_size: Some(size(px(MINIMUM_SIZE.0), px(MINIMUM_SIZE.1))),
                    // Video windows must not drop to GPUI's 30 fps cap when unfocused.
                    inactive_frame_interval: None,
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| RootView::new(store.clone(), window, cx)),
            )
            .expect("open the main window");
            cx.on_window_closed(|cx, _| cx.quit()).detach();
            cx.activate(true);

            let library_options = options.clone();
            let library_store = store.clone();
            cx.subscribe(&store, move |_, event, cx| {
                if *event == StoreEvent::LibraryLoaded {
                    apply_library_options(&library_options, &library_store, window, &root, cx);
                }
            })
            .detach();
            if let Some(path) = options.open.clone() {
                store.update(cx, |store, cx| store.open_folder(path, cx));
            }
            schedule_capture(&options, window, cx);
        });
}
