# teslacam-app: handoff after stage 1

Stage 1 built the app shell, the welcome / scanning / error states, the library panel, the top row and
the stage without video. Stage 2 wires real video (`teslacam-media`), the transport, the expanded and
representative views and the composite export. This note is the map.

## Layout of the crate

| Path | What it is | Tests |
|---|---|---|
| `src/main.rs` | Startup: GPUI Kit + icons (`assets::Assets`), Barlow, `AppTheme`, Aqua appearance, menus (`OpenFolder` ⌘O, `Hide` ⌘H, `Quit` ⌘Q), the 1440 × 900 window (min 1100 × 700, `inactive_frame_interval: None`), development hooks | — |
| `src/store.rs` | `AppStore` entity: `teslacam_core::app_state::AppModel` (state) + its effects (scan, session) | via `ui/tests.rs` |
| `src/scan.rs` | Worker thread running `core::scan::scan_library`, progress channel, result → `AppAction` mapping | unit |
| `src/session.rs` | `Session` of the selected event: `EventTimeline::assuming_full_windows`, `PlaybackMachine`, `ExportMachine` | unit |
| `src/dev.rs` | `TESLACAM_*` development variables (table in the module docs) | unit |
| `src/assets.rs` | Lucide icons (`icon_assets!`), Barlow and `vehicle-top.png` via `include_bytes!` | — |
| `src/presentation/` | Pure logic used by the views: `floor_layout` (where the layers go), `stage_layout` (windows, car, mounts, wedges, grid), `floor_art` (SVG for the floor highlight, the car shadow and the wedges), `motion` (cubic Bézier, the reveal), `numerals` (Barlow runs), `lru` | unit |
| `src/ui/` | GPUI views: `root` (floor + shortcuts), `library` (+ `rail`, `date_picker`, `thumbnails`), `top_row`, `stage`, shared `components` and `style` (tokens as GPUI values, type roles) | `ui/tests.rs` (headless, GPUI Kit `test-support`) |

Rules kept so far: state and rules live in `teslacam-core` (reducer, filters, rail clustering and
placement, calendar grid, every Japanese string, event copy) or in `presentation/`; views only read
and dispatch. Every user-visible string comes from `teslacam_core::copy`.

## Models and data flow

```
AppStore (Entity)                         views observe it (cx.observe) and re-render
 ├─ model: core AppModel ──dispatch(AppAction)──▶ effects:
 │     StartScan{generation}  → scan::spawn_scan(root) → ScanMessage → ScanProgressed / LibraryReady / Scan*
 │     CancelScan{generation} → ScanJob::cancel
 │     StartSession{event_id} → session = Session::new(event, workspace.layout); emit SessionChanged
 │     EndSession             → session = None; emit SessionChanged
 ├─ session: Option<Session>  ← playback(PlaybackInput) / export(ExportAction)
 └─ loaded_at                 ← set on LibraryReady (start of the reveal); emits LibraryLoaded
```

- UI-only state stays in the views: search open, date picker month, open cluster (library), details
  popover and `技術情報` (top row). `StoreEvent::LibraryLoaded` / `SessionChanged` reset them.
- `floor_layout(viewport, floor_phase(store))` is computed independently by each view (pure, cheap),
  so the root, the stage and the top row agree without passing bounds around.
- The rail measures its track with a `canvas` prepaint; a change refreshes the window and
  `core::rail_layout::day_rail` re-clusters for the new width.

## Where stage 2 plugs in

1. **Media session.** In `AppStore::perform(StartSession)` open
   `teslacam_media::EventPlayback::open(&EventInput::from_event(event, &library, &session.timeline), mtm)`
   next to the pure `Session` (keep it in a separate entity, e.g. `SessionMedia`, observed only by the
   stage and the transport: the library and top row must not re-render every video frame).
   `Session::new` already returns the initial `SessionEffects` (autoplay `Play{from, rate}`); they are
   dropped today (`_media` in `store.rs`) — forward them. `AppStore::playback` has the
   `// Stage 2: forward effects.media` hook.
2. **Per-frame loop.** In the stage view, while the session plays (or a command is settling),
   `window.request_animation_frame()`, call `EventPlayback::tick(host_now())`, feed changes back as
   `PlaybackInput::CameraStatus`, `Clock`, `ReachedEnd`, `Started` / `StartFailed`. The window
   already has no inactive frame cap.
3. **Video.** `ui/stage.rs::video_slot` is the dark box inside every window: replace its child with
   `surface(frame.buffer).object_fit(ObjectFit::Contain)` from `EventPlayback::frame(camera, now)`.
   Window states (`TileStatus`) already render: missing / unavailable / corrupt / unsupported overlay
   with stripes; add the loading spinner + `読み込み中` for `TileStatus::Loading` (stage 1 shows a
   plain dark window instead, because nothing ever loads). The reveal (`presentation::motion::Reveal`)
   fades the whole window content, so frames inherit it.
4. **Expanded camera.** `PlaybackMachine` already tracks `expanded_camera()` (window click, digits,
   Esc). Add the expanded layout to `presentation::stage_layout` (main window + strip of five + mini
   car with mount dots) and branch on it in `StageView::render`.
5. **Representative view.** `ViewMode::Representative` renders a placeholder box with
   `代表映像（単独再生）` / `6カメラ同期映像ではありません。`; drive it with
   `teslacam_media::RepresentativePlayer::open(event.mp4)`.
6. **Transport.** `floor_layout` reserves `TRANSPORT_ROOM` (124 pt) under the stage and returns the
   transport rect (`FloorLayout::transport`, max 920 pt, centred on the stage); the root does not
   place anything there yet. Previous / next event = `AppAction::EventStepped`.
7. **Export.** The `書き出す` button dispatches `ExportAction::Open` (or shows the unavailable reason);
   the dialog is not built. Per the coordinator's decisions: the dialog's `書き出しを開始` asks for the
   destination with `cx.prompt_for_new_path` (default name `copy::export_file_name`), runs
   `CompositeExporter`, and on completion the actions pill shows `書き出し完了` with a
   `Finderで表示` button (add the copy to `teslacam_core::copy` first) instead of the web's save
   button. `AppStore::export` has the `// Stage 2` hook for `ExportEffect`s.
8. **Development hooks.** `TESLACAM_PLAY` / `TESLACAM_SEEK` / `TESLACAM_VIEW=expanded:<camera>`
   already reach the pure session; they will drive the media once it is wired.

## Verifying

```bash
cargo test --workspace                  # core 163, media 57 + 16 main-thread, app 31 (4 headless UI)
cargo clippy --workspace --all-targets -- -D warnings
./scripts/gen-fixtures.sh               # media fixtures + the realistic library (~5 s)
cargo build -p teslacam-app
./scripts/capture.sh /tmp/library.png TESLACAM_OPEN="$PWD/.fixtures/TeslaCam" \
  TESLACAM_SELECT=sentry-2026-10-20_20-35-00-1 TESLACAM_UI=details
```

`capture.sh` lets the app capture its own window (`screencapture -l <CGWindowID>`) and quit. Hover
states are whatever the real mouse pointer is over.

## Known gaps after stage 1

- Keyboard reachability: controls have roles and accessible names, but there is no Tab order or
  visible focus ring yet (`DESIGN.md` §9), and the rail's Left/Right/Home/End marker navigation and
  the date picker's arrow keys are missing. Shortcut gating knows the search field only
  (`FocusTarget::TextField`); `FocusTarget::Button` becomes relevant once buttons take focus.
- Motion: the reveal is the only animation, as specified; the details popover opens without the
  320 ms `motion-sheet` transition, and hover colours change instantly (the spec allows it).
- Japanese line breaking: GPUI's line wrapper allows a break before `、` / `。` (no kinsoku), e.g. the
  `invalid` error description at 320 pt wraps as `SavedClips` / `、SentryClips…`.
- Popovers (details, date picker, export reason) use the opaque `surface` instead of `glass`: without
  GPUI blur a translucent layer lets video labels show through.
- The event list's rows are fixed at 88 pt like the web; a missing-camera line that wraps (compact
  library) is tight.
- Not verified by a test: the menu bar and ⌘O open panel (manual only), thumbnail eviction under a
  large library (the LRU bookkeeping is unit-tested, the GPUI cache wrapper is not).
