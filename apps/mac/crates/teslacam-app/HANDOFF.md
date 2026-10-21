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
| `src/presentation/` | Pure logic: `floor_layout` (incl. the message room), `stage_layout` (vehicle, grid, expanded + strip, mini car, representative, `video_rect`), `transport` (scrubber marks and pointer mapping), `floor_art` (floor, car shadow, wedges, progress ring SVG), `bitmap` (overlay → BMP), `motion`, `numerals`, `lru` | unit |
| `src/ui/` | Views: `root`, `library` (+ `rail`, `date_picker`, `thumbnails`), `top_row` (event pill, view switch, export control), `stage`, `transport`, `export_dialog`, `components`, `style` | `ui/tests.rs` (headless) |

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

- **Frame loop.** `StageView::render` asks `AppStore::media_active` (`bridge::frame_loop_needed`:
  probing, running, exporting, the representative video busy, or a seek settling while some camera
  can show video) and schedules one `on_next_frame` tick; the tick calls `media_tick` and notifies
  the stage, so the window redraws at display rate while playing and is idle when paused. The
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

## Verifying

```bash
cargo test --workspace                  # core 167, media 57 + 16 main-thread, app 58 (9 headless UI)
cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --check
./scripts/gen-fixtures.sh               # media fixtures + the realistic library
./scripts/capture.sh /tmp/x.png TESLACAM_OPEN="$PWD/.fixtures/TeslaCam" TESLACAM_SELECT=…  # see README
./scripts/bundle.sh                     # target/bundle/TeslaCam Viewer.app
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

- Keyboard reachability (from stage 1): no Tab order or visible focus ring (`DESIGN.md` §9); the
  rail's arrow-key navigation is missing. The rate menu, the export popovers and the dialog are
  mouse-first (Esc closes the dialog and the rate menu).
- Motion: the reveal is the only animation; popovers and the expanded camera open without the
  `motion-sheet` transition.
- The scrubber has no keyboard control and no hover preview; dragging seeks continuously (each move
  is an exact seek of six players).
- The export dialog's thumbnail draws every slot as video (black) even for a camera with no footage
  in the whole event; the exported video draws `記録なし` there.
- `StartFailed` is never reported: the AVFoundation sync engine starts after its timeouts in every
  case, so `PlaybackMessage::Rejected` cannot appear on the Mac.
- `Finderで表示` and the save panel are verified by hand only (the save panel is skipped in
  development runs with `TESLACAM_EXPORT_TO`; the headless tests stop at the machine's effects).
- 4:3 footage keeps a ~4 pt black rim inside its window (see Video above).
- Not signed or notarized; no app icon (`.icns`) in the bundle yet.
- Japanese line breaking (stage 1): GPUI allows breaks before `、` / `。`.
