//! Fixture events written by `apps/mac/scripts/gen-fixtures.sh`.

use std::path::{Path, PathBuf};

use teslacam_media::{CameraId, CameraMap, EventInput, SegmentInput};

pub const FPS: f64 = 36.0;

pub fn mac_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn saved_clips() -> PathBuf {
    mac_root().join(".fixtures/TeslaCam/SavedClips")
}

/// Generates the fixtures when they are missing or have an older layout (needs ffmpeg).
pub fn ensure() {
    let layout = std::fs::read_to_string(saved_clips().join(".layout")).unwrap_or_default();
    if layout.trim() == "2" {
        return;
    }
    let status = std::process::Command::new(mac_root().join("scripts/gen-fixtures.sh"))
        .status()
        .expect("run scripts/gen-fixtures.sh (needs bash and ffmpeg)");
    assert!(status.success(), "fixture generation failed");
}

pub fn full_dir() -> PathBuf {
    saved_clips().join("2026-10-20_12-01-30")
}

pub fn gaps_dir() -> PathBuf {
    saved_clips().join("2026-10-20_13-00-50")
}

fn event(dir: &Path, segments: &[(&str, f64)]) -> EventInput {
    EventInput {
        segments: segments
            .iter()
            .map(|(timestamp, start)| SegmentInput {
                start_seconds: *start,
                files: CameraMap::from_fn(|camera| {
                    let path = dir.join(format!("{timestamp}-{}.mp4", camera.as_str()));
                    path.exists().then_some(path)
                }),
            })
            .collect(),
    }
}

/// 2 segments × 6 cameras × 60 s.
pub fn full_event() -> EventInput {
    event(
        &full_dir(),
        &[("2026-10-20_12-00-00", 0.0), ("2026-10-20_12-01-00", 60.0)],
    )
}

/// 3 segments × 20 s: no right repeater, no back clip in segment 1, an 8 s front clip in segment 1.
pub fn gaps_event() -> EventInput {
    event(
        &gaps_dir(),
        &[
            ("2026-10-20_13-00-00", 0.0),
            ("2026-10-20_13-00-20", 20.0),
            ("2026-10-20_13-00-40", 40.0),
        ],
    )
}

pub fn representative() -> PathBuf {
    gaps_dir().join("event.mp4")
}

pub fn clip(dir: &Path, timestamp: &str, camera: CameraId) -> PathBuf {
    dir.join(format!("{timestamp}-{}.mp4", camera.as_str()))
}

/// Scratch directory for files the tests create (corrupt clips, exports).
pub fn scratch() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("teslacam-media");
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    dir
}

/// Expected event-wide frame index at event time `t`.
pub fn frame_at(t: f64) -> u32 {
    (t * FPS + 1e-6).floor() as u32
}
