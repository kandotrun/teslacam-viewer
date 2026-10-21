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

fn playback<T>(
    floor: &Floor,
    cx: &mut TestAppContext,
    read: impl FnOnce(&teslacam_core::playback::PlaybackMachine) -> T,
) -> T {
    cx.read(|cx| read(&floor.store.read(cx).session().expect("session").playback))
}

fn click(floor: &Floor, id: impl Into<gpui_kit::ElementId>, cx: &mut TestAppContext) {
    let id = id.into();
    cx.update_window(floor.window, |_, window, cx| {
        window.render_frame(cx);
        window.click(id, cx);
    })
    .expect("click");
    cx.run_until_parked();
}

fn rendered(floor: &Floor, id: &'static str, cx: &mut TestAppContext) -> bool {
    cx.update_window(floor.window, |_, window, cx| {
        window.render_frame(cx);
        window.try_find(id).is_some()
    })
    .expect("render")
}

#[gpui_kit::test]
fn the_transport_plays_skips_scrubs_and_changes_the_rate(cx: &mut TestAppContext) {
    use teslacam_core::playback::PlaybackRate;

    let floor = open_floor(cx);
    assert!(playback(&floor, cx, |playback| playback.intends_to_play()));
    click(&floor, "play", cx);
    assert!(!playback(&floor, cx, |playback| playback.intends_to_play()));
    click(&floor, "skip-forward", cx);
    assert_eq!(position(&floor, cx), 10.0);
    click(&floor, "skip-back", cx);
    assert_eq!(position(&floor, cx), 0.0);

    // The middle of the rail (inset 10 pt from both ends) is the middle of the 60 s event.
    cx.update_window(floor.window, |_, window, cx| {
        window.render_frame(cx);
        let width = window.find("scrubber").bounds().size.width;
        window.click_at("scrubber", Point::new(width / 2.0, px(23.0)), cx);
    })
    .expect("scrub");
    assert!(
        (position(&floor, cx) - 30.0).abs() < 0.5,
        "{}",
        position(&floor, cx)
    );

    assert!(!rendered(&floor, "rate-menu", cx));
    click(&floor, "rate", cx);
    assert!(rendered(&floor, "rate-menu", cx));
    click(&floor, ("rate", 1.5f64.to_bits() as usize), cx);
    assert_eq!(
        playback(&floor, cx, |playback| playback.rate()),
        PlaybackRate::OneAndHalf
    );
    assert!(!rendered(&floor, "rate-menu", cx));
}

#[gpui_kit::test]
fn play_without_a_ready_camera_shows_the_message_under_the_transport(cx: &mut TestAppContext) {
    let floor = open_floor(cx);
    assert!(!rendered(&floor, "playback-message", cx));
    click(&floor, "play", cx);
    click(&floor, "play", cx);
    // Headless tests have no players, so every camera is still loading.
    assert!(rendered(&floor, "playback-message", cx));
    let message = cx.read(|cx| floor.store.read(cx).playback_message());
    assert_eq!(
        message.map(|message| message.text()),
        Some("再生できるカメラ映像がありません。\n読み込み後に再試行してください。")
    );
}

#[gpui_kit::test]
fn the_older_and_newer_buttons_step_through_events(cx: &mut TestAppContext) {
    let floor = open_floor(cx);
    assert_eq!(selected(&floor, cx).as_deref(), Some(RECENT));
    click(&floor, "older-event", cx);
    assert_eq!(selected(&floor, cx).as_deref(), Some(SAVED));
    click(&floor, "newer-event", cx);
    assert_eq!(selected(&floor, cx).as_deref(), Some(RECENT));
}

#[gpui_kit::test]
fn the_mini_car_shows_a_camera_without_collapsing_it(cx: &mut TestAppContext) {
    let floor = open_floor(cx);
    cx.simulate_keystrokes(floor.window, "2");
    assert!(rendered(&floor, "camera-switcher", cx));
    click(&floor, ("show-camera", CameraId::LeftPillar.index()), cx);
    let shown = |cx: &mut TestAppContext| {
        playback(&floor, cx, |playback| {
            (playback.expanded_camera(), playback.active_camera())
        })
    };
    assert_eq!(
        shown(cx),
        (Some(CameraId::LeftPillar), CameraId::LeftPillar)
    );
    click(&floor, ("show-camera", CameraId::LeftPillar.index()), cx);
    assert_eq!(shown(cx).0, Some(CameraId::LeftPillar));
    // The expanded window itself collapses back to the vehicle layout.
    click(&floor, ("camera", CameraId::LeftPillar.index()), cx);
    assert_eq!(shown(cx).0, None);
    assert!(!rendered(&floor, "camera-switcher", cx));
}

#[gpui_kit::test]
fn the_export_dialog_opens_cancels_and_starts(cx: &mut TestAppContext) {
    use teslacam_core::export_state::ExportState;

    let floor = open_floor(cx);
    let state = |cx: &mut TestAppContext| {
        cx.read(|cx| {
            floor
                .store
                .read(cx)
                .session()
                .expect("session")
                .export
                .state()
                .clone()
        })
    };
    click(&floor, "export", cx);
    assert!(rendered(&floor, "export-dialog", cx));
    // Shortcuts wait while the dialog is open; Esc closes it.
    cx.simulate_keystrokes(floor.window, "right");
    assert_eq!(position(&floor, cx), 0.0);
    cx.simulate_keystrokes(floor.window, "escape");
    assert_eq!(state(cx), ExportState::Idle);
    assert!(!rendered(&floor, "export-dialog", cx));

    click(&floor, "export", cx);
    click(&floor, "export-cancel", cx);
    assert_eq!(state(cx), ExportState::Idle);

    let destination = std::path::PathBuf::from("/tmp/TeslaCam-2026-08-20_20-45-00-vehicle.mp4");
    cx.update(|cx| {
        floor.store.update(cx, |store, _| {
            store.set_export_destination(Some(destination.clone()))
        })
    });
    click(&floor, "export", cx);
    click(&floor, "export-start", cx);
    assert_eq!(state(cx), ExportState::Running { progress: 0.0 });
    assert!(!rendered(&floor, "export-dialog", cx));
    assert!(rendered(&floor, "export-progress", cx));
}
