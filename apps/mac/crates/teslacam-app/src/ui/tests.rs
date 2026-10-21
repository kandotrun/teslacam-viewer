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
    open_floor_with(cx, library())
}

fn open_floor_with(cx: &mut TestAppContext, library: Arc<Library>) -> Floor {
    cx.update(|cx| {
        gpui_kit::init(cx);
        AppTheme::install(cx);
        // The motion-sheet transitions run on wall-clock time; tests click windows where they come to rest.
        cx.set_reduce_motion(true);
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
                    library,
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

/// A key press with its release (buttons activate on key-up).
fn press_and_release(floor: &Floor, key: &str, cx: &mut TestAppContext) {
    use gpui_kit::{InputEvent as _, KeyDownEvent, KeyUpEvent, Keystroke};
    let keystroke = Keystroke::parse(key).expect("keystroke");
    cx.update_window(floor.window, |_, window, cx| {
        window.render_frame(cx);
        let down = KeyDownEvent {
            keystroke: keystroke.clone(),
            is_held: false,
            prefer_character_input: false,
        };
        window.dispatch_event(down.to_platform_input(), cx);
        window.dispatch_event(KeyUpEvent { keystroke }.to_platform_input(), cx);
        window.render_frame(cx);
    })
    .expect("press");
    cx.run_until_parked();
}

fn intends_to_play(floor: &Floor, cx: &mut TestAppContext) -> bool {
    playback(floor, cx, |playback| playback.intends_to_play())
}

/// Clicks the bare floor (top-left corner, outside every floating layer).
fn click_floor(floor: &Floor, cx: &mut TestAppContext) {
    cx.update_window(floor.window, |_, window, cx| {
        window.render_frame(cx);
        window.click_at("floor", Point::new(px(2.0), px(2.0)), cx);
    })
    .expect("click the floor");
    cx.run_until_parked();
}

#[gpui_kit::test]
fn a_clicked_transport_button_keeps_the_theater_keys(cx: &mut TestAppContext) {
    // AT-KEY-02 / spec §2.10.10 rule 2: a click focuses the button like on the web.
    let floor = open_floor(cx);
    click(&floor, "play", cx);
    assert!(!intends_to_play(&floor, cx));
    cx.simulate_keystrokes(floor.window, "right");
    assert_eq!(
        position(&floor, cx),
        0.0,
        "→ skipped while the play button had focus"
    );
    cx.simulate_keystrokes(floor.window, "6");
    assert_eq!(
        playback(&floor, cx, |playback| playback.expanded_camera()),
        None
    );
    // Clicking the bare floor gives the keys back to the theater.
    click_floor(&floor, cx);
    cx.simulate_keystrokes(floor.window, "right");
    assert_eq!(position(&floor, cx), 10.0);
}

#[gpui_kit::test]
fn a_clicked_camera_window_ignores_arrows_and_digits_but_escape_collapses(cx: &mut TestAppContext) {
    let floor = open_floor(cx);
    click(&floor, ("camera", CameraId::Front.index()), cx);
    let expanded = |cx: &mut TestAppContext| playback(&floor, cx, |p| p.expanded_camera());
    assert_eq!(expanded(cx), Some(CameraId::Front));
    cx.simulate_keystrokes(floor.window, "6");
    assert_eq!(expanded(cx), Some(CameraId::Front));
    cx.simulate_keystrokes(floor.window, "right");
    assert_eq!(position(&floor, cx), 0.0);
    cx.simulate_keystrokes(floor.window, "escape");
    assert_eq!(expanded(cx), None);
}

#[gpui_kit::test]
fn an_open_rate_menu_keeps_space(cx: &mut TestAppContext) {
    let floor = open_floor(cx);
    cx.update(|cx| {
        let transport = floor.root.read(cx).transport().clone();
        transport.update(cx, |transport, cx| transport.open_rate_menu(cx));
    });
    assert!(rendered(&floor, "rate-menu", cx));
    assert!(intends_to_play(&floor, cx));
    cx.simulate_keystrokes(floor.window, "space");
    assert!(
        intends_to_play(&floor, cx),
        "space reached the theater under the menu"
    );
    cx.simulate_keystrokes(floor.window, "escape");
    assert!(!rendered(&floor, "rate-menu", cx));
}

/// One day with three events far apart: three separate rail markers.
fn spread_library() -> Arc<Library> {
    let files = [
        "TeslaCam/RecentClips/2026-08-20_06-00-00-front.mp4",
        "TeslaCam/SavedClips/noon/2026-08-20_12-00-00-front.mp4",
        "TeslaCam/RecentClips/2026-08-20_20-45-00-front.mp4",
    ]
    .iter()
    .map(|path| FileEntry::new(path.rsplit('/').next().expect("name"), *path, 10))
    .collect();
    Arc::new(build_library(files, |_| {}, |_, _| Ok(None)).expect("library"))
}

fn focused_rail_item(floor: &Floor, cx: &mut TestAppContext) -> Option<usize> {
    cx.update_window(floor.window, |_, window, cx| {
        window.render_frame(cx);
        floor
            .root
            .read(cx)
            .library()
            .read(cx)
            .focused_rail_item(window)
    })
    .expect("read focus")
}

#[gpui_kit::test]
fn arrow_keys_move_focus_along_the_day_rail_without_selecting(cx: &mut TestAppContext) {
    // AT-TL-05: ←/→ move between markers in order, Home/End to the first/last.
    let floor = open_floor_with(cx, spread_library());
    let newest = selected(&floor, cx);
    click(&floor, ("rail-item", 1usize), cx);
    let noon = selected(&floor, cx);
    assert_ne!(noon, newest, "clicking a marker selects its event");
    assert_eq!(focused_rail_item(&floor, cx), Some(1));
    cx.simulate_keystrokes(floor.window, "right");
    assert_eq!(focused_rail_item(&floor, cx), Some(2));
    cx.simulate_keystrokes(floor.window, "right");
    assert_eq!(
        focused_rail_item(&floor, cx),
        Some(2),
        "stays on the last marker"
    );
    cx.simulate_keystrokes(floor.window, "home");
    assert_eq!(focused_rail_item(&floor, cx), Some(0));
    cx.simulate_keystrokes(floor.window, "end");
    assert_eq!(focused_rail_item(&floor, cx), Some(2));
    cx.simulate_keystrokes(floor.window, "left");
    assert_eq!(focused_rail_item(&floor, cx), Some(1));
    assert_eq!(selected(&floor, cx), noon, "focus moves only");
    assert_eq!(position(&floor, cx), 0.0, "the arrows did not skip");
}

#[gpui_kit::test]
fn a_focused_scrubber_moves_with_the_arrow_home_and_end_keys(cx: &mut TestAppContext) {
    let floor = open_floor(cx);
    cx.update_window(floor.window, |_, window, cx| {
        window.render_frame(cx);
        let width = window.find("scrubber").bounds().size.width;
        window.click_at("scrubber", Point::new(width / 2.0, px(23.0)), cx);
    })
    .expect("scrub");
    let start = position(&floor, cx);
    assert!((start - 30.0).abs() < 0.5, "{start}");
    cx.simulate_keystrokes(floor.window, "right");
    assert!((position(&floor, cx) - (start + 10.0)).abs() < 1e-9);
    cx.simulate_keystrokes(floor.window, "end");
    assert_eq!(position(&floor, cx), 60.0);
    cx.simulate_keystrokes(floor.window, "home");
    assert_eq!(position(&floor, cx), 0.0);
    let playing = intends_to_play(&floor, cx);
    cx.simulate_keystrokes(floor.window, "space");
    assert_eq!(
        intends_to_play(&floor, cx),
        playing,
        "space is not play/pause on the slider"
    );
}

#[gpui_kit::test]
fn tab_reaches_the_library_first_and_cycles_inside_the_export_dialog(cx: &mut TestAppContext) {
    let floor = open_floor(cx);
    // The first tab stop is the library's search button: Space opens the search field.
    let search_focused = |cx: &mut TestAppContext| {
        cx.update_window(floor.window, |_, window, cx| {
            window.render_frame(cx);
            let library = floor.root.read(cx).library().clone();
            library.read(cx).search_focus(cx).is_focused(window)
        })
        .expect("read focus")
    };
    cx.simulate_keystrokes(floor.window, "tab");
    assert!(!search_focused(cx));
    press_and_release(&floor, "space", cx);
    assert!(search_focused(cx), "Tab did not start in the library");
    assert!(intends_to_play(&floor, cx), "space went to the theater");
    cx.simulate_keystrokes(floor.window, "escape");

    click(&floor, "export", cx);
    assert!(rendered(&floor, "export-dialog", cx));
    let in_dialog = |cx: &mut TestAppContext| {
        cx.update_window(floor.window, |_, window, cx| {
            window.render_frame(cx);
            floor.root.read(cx).export_dialog_has_focus(window, cx)
        })
        .expect("read focus")
    };
    for press in 0..7 {
        cx.simulate_keystrokes(
            floor.window,
            if press % 3 == 2 { "shift-tab" } else { "tab" },
        );
        assert!(in_dialog(cx), "Tab press {press} left the export dialog");
    }
}

#[gpui_kit::test]
fn the_scan_warnings_stay_at_the_panel_bottom_when_nothing_matches(cx: &mut TestAppContext) {
    let files = [
        "TeslaCam/RecentClips/2026-08-20_20-45-00-front.mp4",
        "TeslaCam/RecentClips/notes.txt",
    ]
    .iter()
    .map(|path| FileEntry::new(path.rsplit('/').next().expect("name"), *path, 10))
    .collect();
    let library = Arc::new(build_library(files, |_| {}, |_, _| Ok(None)).expect("library"));
    let floor = open_floor_with(cx, library);
    let gap = |cx: &mut TestAppContext| {
        cx.update_window(floor.window, |_, window, cx| {
            window.render_frame(cx);
            let panel = window.find("library").bounds();
            let footer = window.find("scan-warnings").bounds();
            (panel.bottom() - footer.bottom()).as_f32()
        })
        .expect("measure")
    };
    let with_rows = gap(cx);
    cx.update_window(floor.window, |_, window, cx| {
        let library = floor.root.read(cx).library().clone();
        library.update(cx, |library, cx| library.open_search("zzz", window, cx));
    })
    .expect("search");
    let without_rows = gap(cx);
    assert!(
        (with_rows - without_rows).abs() < 0.5,
        "the footer moved from {with_rows} to {without_rows} pt above the panel bottom"
    );
}

#[gpui_kit::test]
fn regions_carry_the_catalog_names_shared_with_the_web(cx: &mut TestAppContext) {
    // Spec §5: categories.region, list.region, the day rail region, context.region.
    let floor = open_floor(cx);
    let label = |id: &'static str, cx: &mut TestAppContext| {
        cx.update_window(floor.window, |_, window, cx| {
            window.render_frame(cx);
            window.find(id).label().map(str::to_owned)
        })
        .expect("render")
    };
    assert_eq!(label("categories", cx).as_deref(), Some("映像カテゴリ"));
    assert_eq!(label("event-list", cx).as_deref(), Some("イベント一覧"));
    assert_eq!(
        label("day", cx).as_deref(),
        Some("日内イベントタイムライン")
    );
    assert_eq!(
        label("now-playing", cx).as_deref(),
        Some("再生中のイベント")
    );
}
