# TeslaCam Viewer for macOS

Native macOS version of the TeslaCam Viewer, written in Rust with [GPUI Kit](https://gpui-kit.com/).
It follows the same design system (`DESIGN.md`) and domain contract (`packages/contract`) as the web app.

| Crate | Role |
|---|---|
| `crates/teslacam-core` | Platform-independent domain rules (folder scan, events, timeline, detection), pure state machines and design tokens. Tested against the shared JSON fixtures. |
| `crates/teslacam-media` | AVFoundation layer: six synchronized `AVPlayer`s per event and the composite MP4 export. |
| `crates/teslacam-app` | The GPUI Kit application. |
| `spike/` | The throwaway spike that validated the approach; see `spike/RESULTS.md`. |

Requirements: macOS 14+, Xcode command line tools, Rust 1.96+. The Metal Toolchain is not needed.

```bash
just mac-test
```
