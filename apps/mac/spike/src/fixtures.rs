//! Fixture paths shared by the spike binaries.
use std::path::PathBuf;

pub const CAMERAS: [&str; 6] = [
    "front",
    "back",
    "left_pillar",
    "left_repeater",
    "right_pillar",
    "right_repeater",
];

/// Japanese labels, verbatim from the web app (`cameraLabels`).
pub const CAMERA_LABELS: [&str; 6] = ["フロント", "リア", "左ピラー", "左リピーター", "右ピラー", "右リピーター"];

pub const SEGMENTS: [&str; 2] = ["2026-10-20_12-00-00", "2026-10-20_12-01-00"];

pub fn event_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("SPIKE_EVENT_DIR") {
        return dir.into();
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../.fixtures/TeslaCam/SavedClips/2026-10-20_12-01-30")
}

pub fn clip(segment: usize, camera: usize) -> PathBuf {
    event_dir().join(format!("{}-{}.mp4", SEGMENTS[segment], CAMERAS[camera]))
}

/// Both segments of one camera.
pub fn camera_segments(camera: usize) -> Vec<Option<PathBuf>> {
    (0..SEGMENTS.len()).map(|s| Some(clip(s, camera))).collect()
}

pub fn vehicle_png() -> PathBuf {
    if let Ok(p) = std::env::var("SPIKE_VEHICLE_PNG") {
        return p.into();
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../web/public/vehicle-top.png")
}
