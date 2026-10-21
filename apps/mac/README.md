# TeslaCam Viewer for macOS

Native macOS version of the TeslaCam Viewer, written in Rust with [GPUI Kit](https://gpui-kit.com/).
It follows the same design system (`DESIGN.md`) and domain contract (`packages/contract`) as the web app:
the garage floor with six synchronized camera windows around the car, the day rail, the transport and
the 1280 × 720 composite export.

| Crate | Role |
|---|---|
| `crates/teslacam-core` | Platform-independent domain rules (folder scan, events, timeline, detection), pure state machines (app, playback, export, shortcuts), library presentation (rail, calendar, event copy), every Japanese string and the design tokens. Tested against the shared JSON fixtures. |
| `crates/teslacam-media` | AVFoundation layer: six synchronized `AVPlayer`s per event, the `event.mp4` player and the composite MP4 export. Uses the core camera types and timeline. |
| `crates/teslacam-app` | The GPUI Kit application (see `crates/teslacam-app/HANDOFF.md` for its architecture). |
| `spike/` | The throwaway spike that validated the approach; see `spike/RESULTS.md`. |

## Requirements

- macOS 14 or later, Xcode command line tools, Rust 1.96+. The Metal Toolchain is not needed
  (GPUI Kit compiles its shaders at run time).
- `ffmpeg` and `ffprobe` (Homebrew) for the fixtures and the media tests.

## Run

```bash
just mac-run                      # cargo run --release -p teslacam-app
```

Choose the USB drive's `TeslaCam` folder with `フォルダを選ぶ` or ⌘O. The newest event plays at once;
`Space` plays / pauses, `←` / `→` skip 10 s, `1`–`6` expand a camera, `Esc` returns, `[` / `]` step
through events. `書き出す` exports the event as one 1280 × 720 MP4 (the save panel asks where).

Every control is reachable with `Tab` / `⇧Tab` (library → top row → stage → transport, a 2 pt blue
focus ring) and activates with `Space` or `Return`. While a control has focus — also after clicking it,
as on the web — it keeps the keys, so the shortcuts above wait (`Esc` still collapses a camera); click
the bare floor to give them back. On the day rail `←` / `→` / `Home` / `End` move between markers; on
the scrubber they seek ±10 s or to either end. The export dialog keeps `Tab` inside it.

### App bundle

```bash
./scripts/bundle.sh               # release build + target/bundle/TeslaCam Viewer.app
open "target/bundle/TeslaCam Viewer.app"
```

The bundle (`run.kan.teslacam-viewer`, macOS 14+, high resolution) holds the executable, its
`Info.plist` and `AppIcon.icns` (rendered from `assets/AppIcon.svg` with `sips` and `iconutil`); fonts,
icons and the car image are compiled into the binary. It is neither signed nor
notarized, so a copy moved to another Mac needs right-click > Open the first time.

## Test

```bash
just mac-fixtures                 # .fixtures/TeslaCam: media test events + a realistic library (~1 min)
just mac-test                     # cargo test --workspace
just mac-check                    # fmt + clippy -D warnings
```

- `teslacam-core` and the app's unit and headless UI tests (GPUI Kit `test-support`) need nothing.
- `teslacam-media`'s `media_main_thread` test runs real AVFoundation playback and export against the
  fixtures (it generates them when missing). Its sync and speed checks measure wall-clock timing:
  run it on an otherwise idle machine; under heavy load the 4× sync case can report a few frames of lag.

## Looking at the app without clicking

The app reads `TESLACAM_*` variables for development (table in `crates/teslacam-app/src/dev.rs`):
open a folder, select an event, choose a view, play / pause / seek, open popovers, start an export
without the save panel, delay the players (loading state), press Tab (`TESLACAM_UI=tab:<n>`, shows
the focus ring), log the sync of the drawn frames.
`scripts/capture.sh` runs the debug build, captures its window to a PNG and quits:

```bash
cargo build -p teslacam-app
F="$PWD/.fixtures/TeslaCam"
./scripts/capture.sh /tmp/vehicle.png TESLACAM_OPEN="$F" TESLACAM_SELECT=saved-2026-10-20_12-00-00-3
./scripts/capture.sh /tmp/expanded.png TESLACAM_OPEN="$F" TESLACAM_SELECT=saved-2026-10-20_12-00-00-3 \
  TESLACAM_VIEW=expanded:back TESLACAM_WINDOW=1100x700
./scripts/capture.sh /tmp/export.png TESLACAM_OPEN="$F" TESLACAM_SELECT=saved-2026-10-20_12-00-00-3 \
  TESLACAM_EXPORT_TO=/tmp/out.mp4 TESLACAM_UI=exportprogress TESLACAM_CAPTURE_DELAY_MS=8000
TESLACAM_OPEN="$F" TESLACAM_SELECT=saved-2026-10-20_12-00-00-3 TESLACAM_SEEK=50 \
  TESLACAM_SYNC_PROBE=1 target/debug/teslacam-viewer   # logs "sync: … spread0=… max=…" every 2 s
```

Useful fixture events: `saved-2026-10-20_12-00-00-3` (2 × 60 s, six cameras, frame counters),
`saved-2026-10-20_13-00-00-2` (gaps, a missing camera, `event.mp4`), `sentry-2026-10-19_02-15-02-2`
(Sentry with the pre-roll and a missing left repeater), `sentry-2026-10-20_20-35-00-1`.
