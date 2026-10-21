# teslacam-app: handoff after stage 2

Stage 1 built the shell, the welcome / scanning / error states, the library panel, the top row and
the stage without video. Stage 2 wired the media (`teslacam-media`): six synchronized players drawn
through GPUI surfaces, every camera state, the expanded camera with the mini car, the 6分割 grid, the
representative `event.mp4`, the transport and the composite export with the save panel. This note is
the map of the finished crate and of what is still open.

## Layout of the crate

| Path | What it is | Tests |
|---|---|---|
| `src/main.rs` | Startup: GPUI Kit + icons, Barlow, `AppTheme`, Aqua appearance, menus (⌘O, ⌘H, ⌘Q), the 1440 × 900 window (min 1100 × 700, no inactive frame cap), quit cleanup, development hooks | — |
| `src/store.rs` | `AppStore` entity: core `AppModel` (state) + effects: scan, session, the session's media entity, the frame-loop report, export effects (save panel, start / cancel, Finder) | unit + `ui/tests.rs` |
| `src/session.rs` | `Session` of the selected event: `EventTimeline` (assumed full windows, replaced by the probed one), `PlaybackMachine`, `ExportMachine` | unit |
| `src/media.rs` | `SessionMedia` entity (effects only): `EventPlayback`, the lazily opened `RepresentativePlayer`, the `ExportJob`; `tick(now)` → `MediaReport` | via the app runs (needs the main thread) |
| `src/bridge.rs` | Pure glue: media status → `TileStatus`, status diffs → `PlaybackInput`s, `MediaCommand` → media calls, when the frame loop must run | unit |
| `src/scan.rs`, `src/dev.rs` | Scan worker; `TESLACAM_*` development variables (table in the module docs) | unit |
| `src/sync_probe.rs` | Development: decodes the fixtures' burned-in frame counter from drawn frames, spread statistics | unit |
| `src/assets.rs` | Lucide icons, custom filled play / pause and thin skip icons, Barlow, `vehicle-top.png` | — |
| `src/presentation/` | Pure logic: `floor_layout` (incl. the message room), `stage_layout` (vehicle, grid, expanded + strip, mini car, representative, `video_rect`), `transport` (scrubber marks, pointer mapping, slider keys), `event_pill` (which items fit), `line_break` (Japanese wrapping without stranded tails or a leading `、`/`。`), `floor_art` (floor, car shadow, wedges, progress ring SVG), `bitmap` (overlay → BMP), `motion` (reveal, `motion-sheet`), `numerals`, `lru` | unit |
| `src/ui/` | Views: `root` (shortcuts, focus routing, tab groups), `library` (+ `rail`, `date_picker`, `thumbnails`), `top_row` (event pill, view switch, export control), `stage`, `transport`, `export_dialog`, `components` (incl. `KeyboardFocus` + focus ring), `text` (`ja_text` element), `style` | `ui/tests.rs` (headless) |

State and rules live in `teslacam-core` (playback, export and app machines, copy) or in pure modules
(`presentation/`, `bridge.rs`); views and `media.rs` only read, dispatch and carry out effects. Every
user-visible string comes from `teslacam_core::copy`.

## Models and data flow

```
AppStore (Entity) ── observed by root, library (cached), top row (cached), stage, transport, dialog
 ├─ model: core AppModel ──dispatch(AppAction)──▶ StartScan / CancelScan / StartSession / EndSession
 ├─ session: Session (pure)      ◀─ playback(PlaybackInput): first Clock(media position), then the input
 │                                ─▶ MediaCommands ─▶ media.apply (bridge::media_calls)
 ├─ media: Entity<SessionMedia>  ◀─ media_tick(now) per display frame while needed:
 │     EventPlayback.tick ─▶ MediaReport{loaded(probe, duration), statuses, ended, running, export}
 │       loaded   ─▶ Session::media_loaded (probed timeline, PlaybackInput::Loaded)
 │       statuses ─▶ PlaybackInput::CameraStatus for changed cameras (bridge::status_inputs)
 │       ended    ─▶ ReachedEnd;  running while Waiting ─▶ Started
 │       export   ─▶ ExportAction::Progressed (only when the whole percent changes) / Completed / Failed
 └─ export(ExportAction) ─▶ ExportEffect: ChooseDestination (save panel) / StartExport / CancelExport /
                            RevealOutput (Finder) / DiscardOutput
```

- **Frame loop.** `StageView::render` asks `AppStore::media_active` (`bridge::frame_loop`: every
  display frame while probing, running, exporting, the representative video is busy, or a seek
  settles while some camera can show video; every 250 ms while only a camera is still loading, so a
  late file shows its still frame while paused; idle otherwise) and schedules one tick; the tick
  calls `media_tick` and notifies the stage, so the window redraws at display rate while playing and
  is idle when paused. The
  library and the top row are `cached` views: per-frame redraws cost the root, stage and transport
  only. The per-frame clock is read from the media (transport), never pushed through the store.
- **Video.** `EventPlayback::frame(camera, now)` returns retained `420f` IOSurface buffers that go
  straight to `gpui::surface`. GPUI draws surfaces without rounded corners, so
  `stage_layout::video_rect` fits the frame inside the window and, for 4:3 footage whose corners
  would stick out of the 14 pt rounding, shrinks it until the corners touch the arc (a thin black
  rim remains; wide HW4 footage stays edge to edge). Corner masks in the floor colour were tried
  and dropped: the floating layers' shadows darken the floor unpredictably, so masks showed.
- **Views.** `PlaybackMachine::expanded_camera` → expanded layout (main window, strip of the other
  five in canonical order, mini car with `ShowCamera` dots); `ViewMode::Grid` → 3 × 2; `Representative`
  → `event.mp4` with its own play / time / scrubber controls above a strip of all six, its player
  paused when the view is left. The orchestrated reveal still fades the windows after a folder
  loads (surfaces cannot fade, so video appears once a window has fully revealed).
- **Export (Mac form).** `書き出す` → dialog (thumbnail = `render_overlay` over black, drawn once per
  session on a background thread) → `書き出しを開始` → `ExportEffect::ChooseDestination` → save panel
  in `~/Movies` named `TeslaCam-{timestamp}-vehicle.mp4` → `DestinationChosen` → `CompositeExporter`.
  The actions pill shows the ring + `書き出し中 42%` (popover: clock, bar, `中止`), then
  `書き出し完了` + `Finderで表示` + close; failures show `もう一度` and the message, aborts the notice.
  The finished file is the user's: closing or ending the session never deletes it. Quitting during an
  export cancels it and waits (up to 2 s) so the partial file and AVFoundation's temp file are removed.

- **Keyboard.** Every control is a tab stop (`KeyboardFocus::keyboard_focus`: `tab_index(0)` + a
  2 pt `drive` ring 3 pt outside, shown for keyboard focus only). A click focuses the control like a
  button on the web, and `RootView::focus_target` then reports `FocusTarget::Button` (any focused
  element but the floor) or `TextField` (search field, or an open popover / menu), so the theater
  shortcuts wait (spec §2.10.10 rule 2, AT-KEY-02); a click on the bare floor focuses the floor again.
  Tab / Shift-Tab are GPUI Kit's root bindings; the floating layers are tab groups (library 1, top
  row 2, stage 3, transport 4, dialog 5). The export dialog is a GPUI Kit focus trap: it takes focus
  when it opens and gives it back to `書き出す` when dismissed. The day rail moves focus with
  `←`/`→`/`Home`/`End` (`rail::rail_focus_target`, handles owned by the library); the scrubber is a
  focusable slider (`aria_value` like `0分15秒 / 1分`) that seeks ±10 s / to the ends.
- **Motion.** Besides the reveal, expanding or collapsing a camera moves the windows from their last
  drawn rects over `motion-sheet` (320 ms), and the details popover fades in and settles 8 pt down;
  "Reduce motion" makes both instant (`presentation::motion::Sheet`).
- **Japanese text.** Warnings, error paragraphs and popover messages use `ui::text::ja_text`, which
  measures with the text system and breaks with `presentation::line_break` (ASCII words whole, no
  line starting with closing punctuation or a small kana, at least three characters on the last line).
  The event pill drops the detection, then the place, whole when they do not fit (measured text,
  `presentation::event_pill`).

## Verifying

```bash
cargo test --workspace                  # core 171, media 58 + 18 main-thread, app 80 (19 headless UI)
cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check
./scripts/gen-fixtures.sh               # media fixtures + the realistic library
./scripts/capture.sh /tmp/x.png TESLACAM_OPEN="$PWD/.fixtures/TeslaCam" TESLACAM_SELECT=…  # see README
./scripts/bundle.sh                     # target/bundle/TeslaCam Viewer.app (with AppIcon.icns)
```

Measured on the development machine (Apple M4 Max, heavily loaded by other work during the runs):

- Sync: `TESLACAM_SYNC_PROBE=1` on `saved-2026-10-20_12-00-00-3` from 50 s, 22 s across the 60 s
  segment boundary: 1,186 drawn samples (~60 per second), every one with all six counters equal
  (spread 0, max 0).
- CPU (release, 6 × 1280 × 960 @ 36 fps): playing 3.7–9.1 % of one core over 20 s windows (five
  runs), paused 0.0 %, welcome 0.1 %, an event whose every file is broken 0.0 %; RSS ~280 MB.
- Export from the app: `TeslaCam-…-vehicle.mp4`, H.264 Main 1280 × 720, 30 fps, 3,600 frames, 120.0 s,
  ~7.3 Mbit/s; the frame at 75 s shows `frame 2700` on all six cameras (75 × 36). Speed depends on
  the load: one run finished inside a 55 s capture window (launch, scan and probe included), one run
  during a load spike (load average 68) took 138.7 s; the media test measures 3.0× (full) and 5.2×
  (gaps event) real time.

## Known gaps

- The rate menu, the date picker and the details popover open from the keyboard but their items are
  plain tab stops: no arrow-key roving inside a menu, and focus does not move into a popover when it
  opens (the export dialog does).
- The scrubber has no hover preview; dragging seeks continuously (each move is an exact seek of six
  players).
- `StartFailed` is never reported: the AVFoundation sync engine starts after its timeouts in every
  case, so `PlaybackMessage::Rejected` cannot appear on the Mac.
- `Finderで表示` and the save panel are verified by hand only (the save panel is skipped in
  development runs with `TESLACAM_EXPORT_TO`; the headless tests stop at the machine's effects). The
  finished state offers `Finderで表示` where `DESIGN.md` §6/§8 and spec §2.11 still say `MP4を保存`
  (the destination is chosen up front on the Mac); the documents have not recorded this yet.
- 4:3 footage keeps a ~4 pt black rim inside its window (see Video above).
- Not signed or notarized.
- `ja_text` handles single-style text; mixed runs (Barlow numerals inside a sentence) still use GPUI's
  wrapping, which may break anywhere.
- The `copy-format.json` fixture does not pin the UI-label formatters (cluster, category option, day
  button, date picker, export and rate labels): the web keeps them inline in components, so adding
  cases needs those formatters exported on the web first.
