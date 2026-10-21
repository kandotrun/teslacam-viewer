# GPUI + AVFoundation spike results (2026-10-20)

Throwaway code that proved the risky parts of the Mac app before the real crates were written.
Machine: Apple M4 Max, macOS, Xcode 27.1 without the Metal Toolchain component.
Fixtures (`scripts/gen-fixtures.sh`, written to `apps/mac/.fixtures/`): 2 segments × 6 cameras × 60 s,
H.264 1280×960 @ 36 fps, a binary frame counter burned into every frame.

| Question | Result |
|---|---|
| Build without `xcrun metal` | Works. `gpui-kit =0.8.0` pins `gpui-pre =0.3.9` and enables `runtime_shaders`. |
| Window, GPUI Kit components, Japanese text | Works (`src/bin/tiles.rs`). |
| AVPlayerItemVideoOutput → `surface()` | Works. Buffers are `420f` (NV12 full range), IOSurface-backed, 1280×960. Six tiles render at 60 renders/s. Keep recent buffers alive for a few frames (gpui's Metal `draw_surfaces` drops the `CVMetalTexture` before the command buffer completes). |
| Sync: 6 AVPlayers, shared host clock, `setRate:time:atHostTime:` | 0 frames of spread at 0.5×, 1×, 2× and 4× in steady playback (`sync_probe`). With a frame-ready wait after seeking, 0 frames of spread after seeks, including across the 60 s segment boundary; all tiles on target ~9 ms after a seek. Naive independent `play()` reaches 125–3,851 frames of spread after seeks. |
| Per-camera event track (AVMutableComposition of segments) | Works, continuous across the boundary. |
| Composite export (AVMutableComposition + AVMutableVideoComposition + CoreAnimation overlay → AVAssetExportSession) | 120 s event exported in 28.3 s (4.2× real time), H.264 Main 1280×720 30 fps, 3,600 frames, every sampled frame shows the same counter on all six cameras. Forcing per-frame `CATextLayer` display dropped it to 1.5× → render the overlay (labels, car, masks) into one static image instead. |
| Runtime cost | 6 × 1280×960 playback: 10–12 % CPU of one core with ~120 MB RSS; paused ≈ 0.3–1 %. GPUI caps inactive windows at 30 fps by default; disable that cap for video windows. Drive repaints with `window.request_animation_frame()` only while playing. |
| `screencapture -l <window>` | Works once the window is on screen (`window.png`); earlier attempts returned a blank image. |
| Headless UI tests | `gpui-kit` `test-support` (`TestAppContext`) and `gpui-testing-library 0.1.0` (without its `gpui-kit` feature) both run under `cargo test`. AVFoundation tests need the main thread (`harness = false`). |
