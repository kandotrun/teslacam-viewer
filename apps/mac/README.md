# TeslaCam Viewer for macOS

Native macOS version of the TeslaCam Viewer, written in Rust with [GPUI Kit](https://gpui-kit.com/).
It follows the same design system (`DESIGN.md`) and domain contract (`packages/contract`) as the web app.

| Crate | Role |
|---|---|
| `crates/teslacam-core` | Platform-independent domain rules (folder scan, events, timeline, detection), pure state machines, library presentation (rail, calendar, event copy) and design tokens. Tested against the shared JSON fixtures. |
| `crates/teslacam-media` | AVFoundation layer: six synchronized `AVPlayer`s per event and the composite MP4 export. Uses the core camera types and timeline. |
| `crates/teslacam-app` | The GPUI Kit application (see `crates/teslacam-app/HANDOFF.md` for its architecture). |
| `spike/` | The throwaway spike that validated the approach; see `spike/RESULTS.md`. |

Requirements: macOS 14+, Xcode command line tools, Rust 1.96+. The Metal Toolchain is not needed.
Media tests and fixtures need `ffmpeg`.

```bash
just mac-test                     # cargo test --workspace
just mac-check                    # fmt + clippy -D warnings
just mac-fixtures                 # .fixtures/TeslaCam: media test events + a realistic library
just mac-run                      # the app
```

## Looking at the app without clicking

The app reads `TESLACAM_*` variables for development (documented in
`crates/teslacam-app/src/dev.rs`), and `scripts/capture.sh` captures its window to a PNG:

```bash
cargo build -p teslacam-app
./scripts/capture.sh /tmp/library.png TESLACAM_OPEN="$PWD/.fixtures/TeslaCam" \
  TESLACAM_SELECT=sentry-2026-10-20_20-35-00-1 TESLACAM_UI=details TESLACAM_WINDOW=1100x700
```
