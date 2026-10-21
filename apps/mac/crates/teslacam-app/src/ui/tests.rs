//! Headless UI tests (GPUI Kit `test-support`): the real views in a test window, driven by key
//! presses and clicks, checked against the store's state.

use std::sync::Arc;
use std::time::Duration;

use gpui_kit::test::TestWindowExt as _;
use gpui_kit::{
    AnyWindowHandle, AppContext as _, Bounds, Entity, Point, TestAppContext, WindowBounds,
    WindowOptions, px, size,
};
use teslacam_core::app_state::{AppAction, AppState};
use teslacam_core::camera::CameraId;
use teslacam_core::library::{FileEntry, Library, build_library};

use super::root::RootView;
use super::style::AppTheme;
use crate::store::AppStore;

const RECENT: &str = "recent-2026-08-20_20-45-00-1";
const SAVED: &str = "saved-2026-08-20_20-40-00-1";
const SENTRY: &str = "sentry-2026-08-20_20-35-00-1";

fn library() -> Arc<Library> {
    let files = [
        "TeslaCam/RecentClips/2026-08-20_20-45-00-front.mp4",
        "TeslaCam/SavedClips/saved/2026-08-20_20-40-00-front.mp4",
        "TeslaCam/SentryClips/sentry/2026-08-20_20-35-00-front.mp4",
        "TeslaCam/SavedClips/older/2026-08-19_19-30-00-front.mp4",
    ]
    .iter()
    .map(|path| FileEntry::new(path.rsplit('/').next().expect("name"), *path, 10))
    .collect();
    Arc::new(build_library(files, |_| {}, |_, _| Ok(None)).expect("library"))
}

struct Floor {
    store: Entity<AppStore>,
    root: Entity<RootView>,
    window: AnyWindowHandle,
}

fn open_floor(cx: &mut TestAppContext) -> Floor {
    cx.update(|cx| {
        gpui_kit::init(cx);
        AppTheme::install(cx);
    });
    let store = cx.new(|_| AppStore::new(Duration::ZERO));
    let (window, root) = cx.update(|cx| {
        let bounds = Bounds {
            origin: Point::default(),
            size: size(px(1440.0), px(900.0)),
        };
        gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| RootView::new(store.clone(), window, cx)),
        )
        .expect("open the test window")
    });
    cx.update(|cx| {
        store.update(cx, |store, cx| {
            // No folder is set, so the scan effect does nothing; the library arrives directly.
            store.dispatch(AppAction::ScanStarted, cx);
            store.dispatch(
                AppAction::LibraryReady {
                    generation: 1,
                    folder_name: "TeslaCam".into(),
                    library: library(),
                },
                cx,
            );
        });
    });
    cx.update_window(window, |_, window, cx| window.render_frame(cx))
        .expect("render");
    Floor {
        store,
        root,
        window,
    }
}

fn selected(floor: &Floor, cx: &mut TestAppContext) -> Option<String> {
    cx.read(|cx| match floor.store.read(cx).state() {
        AppState::Ready(workspace) => workspace.selected_event_id.clone(),
        _ => None,
    })
}

fn position(floor: &Floor, cx: &mut TestAppContext) -> f64 {
    cx.read(|cx| {
        floor
            .store
            .read(cx)
            .session()
            .expect("session")
            .playback
            .position()
    })
}

#[gpui_kit::test]
fn brackets_step_through_events_and_modified_keys_are_ignored(cx: &mut TestAppContext) {
    let floor = open_floor(cx);
    assert_eq!(selected(&floor, cx).as_deref(), Some(RECENT));
    cx.simulate_keystrokes(floor.window, "[");
    assert_eq!(selected(&floor, cx).as_deref(), Some(SAVED));
    cx.simulate_keystrokes(floor.window, "[");
    assert_eq!(selected(&floor, cx).as_deref(), Some(SENTRY));
    cx.simulate_keystrokes(floor.window, "]");
    assert_eq!(selected(&floor, cx).as_deref(), Some(SAVED));
    for modified in ["cmd-]", "ctrl-]", "alt-]"] {
        cx.simulate_keystrokes(floor.window, modified);
        assert_eq!(selected(&floor, cx).as_deref(), Some(SAVED), "{modified}");
    }
}

#[gpui_kit::test]
fn space_digits_arrows_and_escape_drive_the_session(cx: &mut TestAppContext) {
    let floor = open_floor(cx);
    let session = |cx: &mut TestAppContext| {
        cx.read(|cx| {
            let session = floor.store.read(cx).session().expect("session");
            (
                session.playback.intends_to_play(),
                session.playback.expanded_camera(),
            )
        })
    };
    assert_eq!(session(cx), (true, None));
    cx.simulate_keystrokes(floor.window, "space");
    assert_eq!(session(cx), (false, None));
    cx.simulate_keystrokes(floor.window, "2");
    assert_eq!(session(cx), (false, Some(CameraId::Back)));
    cx.simulate_keystrokes(floor.window, "escape");
    assert_eq!(session(cx), (false, None));
    cx.simulate_keystrokes(floor.window, "right");
    assert_eq!(position(&floor, cx), 10.0);
    cx.simulate_keystrokes(floor.window, "left");
    assert_eq!(position(&floor, cx), 0.0);
}

#[gpui_kit::test]
fn shortcuts_wait_while_the_search_field_has_focus(cx: &mut TestAppContext) {
    let floor = open_floor(cx);
    cx.update_window(floor.window, |_, window, cx| {
        let library = floor.root.read(cx).library().clone();
        library.update(cx, |library, cx| library.open_search("", window, cx));
        let focus = library.read(cx).search_focus(cx);
        window.focus(&focus, cx);
        window.render_frame(cx);
    })
    .expect("focus the search field");
    cx.simulate_keystrokes(floor.window, "right");
    assert_eq!(position(&floor, cx), 0.0);
    cx.simulate_keystrokes(floor.window, "space");
    let playing = cx.read(|cx| {
        floor
            .store
            .read(cx)
            .session()
            .expect("session")
            .playback
            .intends_to_play()
    });
    assert!(playing, "space typed into the field, not play/pause");
    let query = cx.read(|cx| match floor.store.read(cx).state() {
        AppState::Ready(workspace) => workspace.filters.query.clone(),
        _ => String::new(),
    });
    assert_eq!(query, " ", "the keys went to the search field");
    assert_eq!(selected(&floor, cx).as_deref(), Some(RECENT));
}

#[gpui_kit::test]
fn the_day_buttons_move_between_recorded_days(cx: &mut TestAppContext) {
    let floor = open_floor(cx);
    cx.update_window(floor.window, |_, window, cx| {
        window.click("previous-day", cx);
    })
    .expect("click");
    assert_eq!(
        selected(&floor, cx).as_deref(),
        Some("saved-2026-08-19_19-30-00-2")
    );
    cx.update_window(floor.window, |_, window, cx| {
        window.render_frame(cx);
        window.click("next-day", cx);
    })
    .expect("click");
    assert_eq!(selected(&floor, cx).as_deref(), Some(RECENT));
}
