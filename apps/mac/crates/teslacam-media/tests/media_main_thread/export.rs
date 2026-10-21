//! Composite export against real media: format, timing, sync of the six slots, overlay pixels.

use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    process::Command,
    rc::Rc,
    time::{Duration, Instant},
};

use objc2::MainThreadMarker;
use teslacam_media::{
    CameraId, CameraMap, CompositeExporter, EventInput, ExportError, ExportJob, ExportState,
    SegmentInput,
    layout::{ExportLayout, Rect},
    run_loop::pump_until,
};

use crate::{
    counter::{Luma, ORIGIN, Stamp, decode_at},
    fixtures::{self, FPS},
};

pub const TESTS: &[crate::Test] = &[
    (
        "export_full_event_is_1280x720_30fps_in_sync_and_fast",
        export_full_event_is_1280x720_30fps_in_sync_and_fast,
    ),
    (
        "export_shows_missing_tiles_only_while_a_camera_has_no_video",
        export_shows_missing_tiles_only_while_a_camera_has_no_video,
    ),
    (
        "export_can_be_cancelled_and_leaves_no_file",
        export_can_be_cancelled_and_leaves_no_file,
    ),
    (
        "export_errors_map_to_the_contract",
        export_errors_map_to_the_contract,
    ),
];

const W: usize = 1280;
const H: usize = 720;
/// 1280 × 960 sources contain-fitted into 352 × 264 (and 320 × 240 for the back camera).
fn scale(camera: CameraId) -> f64 {
    if camera == CameraId::Back {
        0.25
    } else {
        0.275
    }
}

fn run(job: &mut ExportJob, timeout: Duration) -> ExportState {
    assert!(
        pump_until(timeout, || job.poll().is_done()),
        "export did not finish"
    );
    job.state().clone()
}

fn export(
    event: &EventInput,
    name: &str,
    mtm: MainThreadMarker,
) -> (PathBuf, ExportState, Vec<f32>, f64) {
    let output = fixtures::scratch().join(name);
    let progress = Rc::new(RefCell::new(Vec::new()));
    let sink = progress.clone();
    let started = Instant::now();
    let mut job =
        CompositeExporter::new().start(event, &output, move |p| sink.borrow_mut().push(p), mtm);
    let state = run(&mut job, Duration::from_secs(300));
    let elapsed = started.elapsed().as_secs_f64();
    let progress = progress.borrow().clone();
    (output, state, progress, elapsed)
}

#[derive(Debug, PartialEq)]
struct Probe {
    codec: String,
    width: usize,
    height: usize,
    rate: String,
    frames: u64,
}

fn ffprobe(path: &Path) -> Probe {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-count_frames",
            "-show_entries",
        ])
        .arg("stream=codec_name,width,height,r_frame_rate,nb_read_frames")
        .args(["-of", "default=nw=1"])
        .arg(path)
        .output()
        .expect("ffprobe");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let field = |name: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(&format!("{name}=")))
            .unwrap_or_default()
            .to_string()
    };
    Probe {
        codec: field("codec_name"),
        width: field("width").parse().unwrap_or(0),
        height: field("height").parse().unwrap_or(0),
        rate: field("r_frame_rate"),
        frames: field("nb_read_frames").parse().unwrap_or(0),
    }
}

/// Decodes every `nth` frame as 8-bit grey.
fn grey_frames(path: &Path, nth: usize) -> Vec<Vec<u8>> {
    let out = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(path)
        .args([
            "-vf",
            &format!("select='not(mod(n\\,{nth}))',format=gray"),
            "-fps_mode",
            "passthrough",
        ])
        .args(["-f", "rawvideo", "-"])
        .output()
        .expect("ffmpeg");
    out.stdout.chunks_exact(W * H).map(<[u8]>::to_vec).collect()
}

/// Decodes frame `index` as RGB24.
fn rgb_frame(path: &Path, index: u64) -> Vec<u8> {
    let out = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-i"])
        .arg(path)
        .args([
            "-vf",
            &format!("select='eq(n\\,{index})'"),
            "-fps_mode",
            "passthrough",
            "-frames:v",
            "1",
        ])
        .args(["-pix_fmt", "rgb24", "-f", "rawvideo", "-"])
        .output()
        .expect("ffmpeg");
    assert_eq!(
        out.stdout.len(),
        W * H * 3,
        "frame {index} of {}",
        path.display()
    );
    out.stdout
}

fn save_png(path: &Path, index: u64, png: &Path) {
    let status = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(path)
        .args([
            "-vf",
            &format!("select='eq(n\\,{index})'"),
            "-fps_mode",
            "passthrough",
            "-frames:v",
            "1",
        ])
        .arg(png)
        .status()
        .expect("ffmpeg");
    assert!(status.success());
}

fn px(rgb: &[u8], x: f64, y: f64) -> [u8; 3] {
    let i = (y as usize * W + x as usize) * 3;
    [rgb[i], rgb[i + 1], rgb[i + 2]]
}

fn close(a: [u8; 3], b: [u8; 3], tolerance: u8) -> bool {
    a.iter().zip(b).all(|(x, y)| x.abs_diff(y) <= tolerance)
}

/// The mean RGB of a 5 × 5 patch.
fn patch(rgb: &[u8], x: f64, y: f64) -> [u8; 3] {
    let mut sum = [0u32; 3];
    for dy in -2..=2 {
        for dx in -2..=2 {
            let p = px(rgb, x + f64::from(dx), y + f64::from(dy));
            for c in 0..3 {
                sum[c] += u32::from(p[c]);
            }
        }
    }
    sum.map(|s| (s / 25) as u8)
}

fn slot_stamp(grey: &[u8], layout: &ExportLayout, camera: CameraId) -> Option<Stamp> {
    let slot = layout.slots[camera];
    let s = scale(camera);
    let luma = Luma {
        data: grey,
        stride: W,
        width: W,
        height: H,
    };
    decode_at(&luma, slot.x + ORIGIN.0 * s, slot.y + ORIGIN.1 * s, s)
}

const FLOOR: [u8; 3] = [0xE9, 0xEB, 0xEE];
const MISSING: [u8; 3] = [0x15, 0x18, 0x1D];

fn export_full_event_is_1280x720_30fps_in_sync_and_fast(mtm: MainThreadMarker) {
    let (output, state, progress, elapsed) = export(&fixtures::full_event(), "full.mp4", mtm);
    let ExportState::Finished {
        path,
        duration,
        frames,
    } = state
    else {
        panic!("{state:?}")
    };
    assert_eq!(path, output);
    assert!((duration - 120.0).abs() < 1e-3);
    assert_eq!(frames, 3600);
    let x_realtime = duration / elapsed;
    println!("  exported {duration:.1} s in {elapsed:.2} s = {x_realtime:.2}x real time");
    assert!(x_realtime >= 3.0, "export too slow: {x_realtime:.2}x");

    assert_eq!(progress.first(), Some(&0.0));
    assert_eq!(progress.last(), Some(&1.0));
    assert!(
        progress.windows(2).all(|w| w[0] < w[1]),
        "progress not increasing: {progress:?}"
    );
    assert!(progress.len() > 3, "too few progress reports: {progress:?}");

    let probe = ffprobe(&output);
    println!(
        "  {probe:?}, {} bytes",
        std::fs::metadata(&output).unwrap().len()
    );
    assert_eq!(
        probe,
        Probe {
            codec: "h264".into(),
            width: 1280,
            height: 720,
            rate: "30/1".into(),
            frames: 3600
        }
    );

    // Every 15th frame: all six slots show the same source frame, the one due at that output time.
    let layout = ExportLayout::contract();
    let (mut checked, mut lag_max) = (0, 0i64);
    for (sample, grey) in grey_frames(&output, 15).iter().enumerate() {
        let index = sample as u64 * 15;
        let stamps: CameraMap<Option<Stamp>> =
            CameraMap::from_fn(|camera| slot_stamp(grey, &layout, camera));
        for (camera, stamp) in stamps.iter() {
            let stamp =
                stamp.unwrap_or_else(|| panic!("frame {index}: {camera:?} counter unreadable"));
            assert_eq!(
                stamp.id as usize,
                camera.index() + 1,
                "frame {index}: {camera:?} in the wrong slot"
            );
        }
        let values: Vec<u32> = stamps.iter().map(|(_, s)| s.unwrap().frame).collect();
        assert!(
            values.iter().all(|v| *v == values[0]),
            "frame {index}: slots out of sync {values:?}"
        );
        let due = (index as f64 / 30.0 * FPS + 1e-6).floor() as i64;
        lag_max = lag_max.max((due - i64::from(values[0])).abs());
        checked += 1;
    }
    println!("  {checked} sampled frames in sync; max |due − shown| = {lag_max} source frames");
    assert_eq!(checked, 240);
    assert!(lag_max <= 1);
    save_png(
        &output,
        61 * 30,
        &fixtures::scratch().join("export-full-61s.png"),
    );
}

fn export_shows_missing_tiles_only_while_a_camera_has_no_video(mtm: MainThreadMarker) {
    let (output, state, _, elapsed) = export(&fixtures::gaps_event(), "gaps.mp4", mtm);
    let ExportState::Finished {
        frames, duration, ..
    } = state
    else {
        panic!("{state:?}")
    };
    println!(
        "  exported {duration:.1} s in {elapsed:.2} s = {:.2}x real time",
        duration / elapsed
    );
    assert_eq!(frames, 1800);
    assert_eq!(ffprobe(&output).frames, 1800);
    save_png(
        &output,
        30 * 30,
        &fixtures::scratch().join("export-gaps-30s.png"),
    );
    let layout = ExportLayout::contract();
    let centre = |camera: CameraId| layout.slots[camera].center();

    // (time, cameras showing the missing tile, cameras showing video)
    let cases: [(f64, &[CameraId], &[CameraId]); 4] = [
        (
            10.0,
            &[CameraId::RightRepeater],
            &[CameraId::Front, CameraId::Back, CameraId::LeftPillar],
        ),
        (
            25.0,
            &[CameraId::RightRepeater, CameraId::Back],
            &[CameraId::Front, CameraId::RightPillar],
        ),
        (
            30.0,
            &[CameraId::RightRepeater, CameraId::Back, CameraId::Front],
            &[CameraId::LeftRepeater],
        ),
        (
            50.0,
            &[CameraId::RightRepeater],
            &[CameraId::Front, CameraId::Back],
        ),
    ];
    for (t, missing, video) in cases {
        let index = (t * 30.0) as u64;
        let rgb = rgb_frame(&output, index);
        let grey: Vec<u8> = rgb
            .chunks_exact(3)
            .map(|p| {
                ((u32::from(p[0]) * 77 + u32::from(p[1]) * 150 + u32::from(p[2]) * 29) >> 8) as u8
            })
            .collect();
        // The floor around the frame.
        for (x, y) in [(4.0, 4.0), (1275.0, 715.0), (640.0, 708.0), (30.0, 420.0)] {
            assert!(
                close(patch(&rgb, x, y), FLOOR, 6),
                "t {t}: floor at ({x}, {y}) is {:?}",
                patch(&rgb, x, y)
            );
        }
        for camera in missing {
            let (cx, cy) = centre(*camera);
            // Above the centred message: the missing fill.
            let fill = patch(&rgb, cx, cy - 40.0);
            assert!(
                close(fill, MISSING, 8),
                "t {t}: {camera:?} missing fill {fill:?}"
            );
            assert!(
                slot_stamp(&grey, &layout, *camera).is_none(),
                "t {t}: {camera:?} shows video"
            );
            let bright = (-40..40)
                .filter(|dx| px(&rgb, cx + f64::from(*dx), cy)[0] > 0x80)
                .count();
            assert!(bright > 3, "t {t}: {camera:?} has no 記録なし text");
        }
        for camera in video {
            let stamp = slot_stamp(&grey, &layout, *camera)
                .unwrap_or_else(|| panic!("t {t}: {camera:?} no video"));
            assert_eq!(stamp.id as usize, camera.index() + 1);
            assert!(
                (i64::from(stamp.frame) - fixtures::frame_at(t) as i64).abs() <= 1,
                "t {t}: {camera:?} {stamp:?}"
            );
        }
        for camera in CameraId::ALL {
            let slot = layout.slots[camera];
            // Rounded 12 px corners: the outermost corner pixels are floor (H.264 chroma bleeds a
            // little from the video next to them, hence the tolerance).
            for (x, y) in [
                (slot.x + 1.0, slot.bottom() - 2.0),
                (slot.right() - 2.0, slot.y + 1.0),
            ] {
                let corner = px(&rgb, x, y);
                assert!(
                    close(corner, FLOOR, 16),
                    "t {t}: {camera:?} corner ({x}, {y}) {corner:?}"
                );
            }
            // Outside the left / right edge: floor (or a faint wedge on it).
            for x in [slot.x - 4.0, slot.right() + 4.0] {
                let edge = patch(&rgb, x, slot.y + 30.0);
                assert!(
                    close(edge, FLOOR, 12),
                    "t {t}: {camera:?} outside edge {edge:?}"
                );
            }
            // The label pill: a dark scrim at its left padding, white text inside.
            let pill = Rect::new(slot.x + 10.0, slot.bottom() - 34.0, 40.0, 24.0);
            // 55 % black over the brightest video leaves at most 0.45 × 255 ≈ 115 (plus codec noise).
            let scrim = px(&rgb, pill.x + 5.0, pill.y + 12.0);
            assert!(
                scrim.iter().all(|c| *c <= 135),
                "t {t}: {camera:?} scrim {scrim:?}"
            );
            let white = (0..24)
                .flat_map(|dy| (12..48).map(move |dx| (dx, dy)))
                .filter(|(dx, dy)| {
                    px(&rgb, pill.x + f64::from(*dx), pill.y + f64::from(*dy))
                        .iter()
                        .all(|c| *c > 0xC8)
                })
                .count();
            assert!(
                white > 8,
                "t {t}: {camera:?} label text missing ({white} white pixels)"
            );
        }
    }
}

fn export_can_be_cancelled_and_leaves_no_file(mtm: MainThreadMarker) {
    let output = fixtures::scratch().join("cancelled.mp4");
    let mut job = CompositeExporter::new().start(&fixtures::full_event(), &output, |_| {}, mtm);
    assert!(pump_until(
        Duration::from_secs(60),
        || matches!(job.poll(), ExportState::Exporting { progress } if *progress > 0.05)
    ));
    job.cancel();
    let state = run(&mut job, Duration::from_secs(30));
    assert_eq!(state, ExportState::Failed(ExportError::Aborted));
    assert!(!output.exists(), "partial output left behind");

    // Cancelling while preparing stops at once, before any encoding.
    let mut job = CompositeExporter::new().start(&fixtures::full_event(), &output, |_| {}, mtm);
    job.cancel();
    assert_eq!(job.poll(), &ExportState::Failed(ExportError::Aborted));
    assert!(!output.exists());
}

fn export_errors_map_to_the_contract(mtm: MainThreadMarker) {
    let scratch = fixtures::scratch();
    let garbage = scratch.join("garbage-export.mp4");
    std::fs::write(&garbage, b"\0\x01\x02\x03").unwrap();
    let nothing = EventInput {
        segments: vec![SegmentInput {
            start_seconds: 0.0,
            files: CameraMap::from_fn(|c| (c == CameraId::Front).then(|| garbage.clone())),
        }],
    };
    let mut job =
        CompositeExporter::new().start(&nothing, &scratch.join("nothing.mp4"), |_| {}, mtm);
    let state = run(&mut job, Duration::from_secs(30));
    assert_eq!(state, ExportState::Failed(ExportError::NoPlayableMedia));
    assert_eq!(
        ExportError::NoPlayableMedia.message(),
        "再生可能な動画がありません。"
    );
    assert_eq!(ExportError::NoPlayableMedia.code(), "no-playable-media");

    let event = fixtures::gaps_event();
    let mut job = CompositeExporter::new()
        .with_vehicle_png(b"not a png".to_vec())
        .start(&event, &scratch.join("car.mp4"), |_| {}, mtm);
    let state = run(&mut job, Duration::from_secs(30));
    assert_eq!(state, ExportState::Failed(ExportError::VehicleImage));
    assert_eq!(
        ExportError::VehicleImage.message(),
        "車両イメージを読み込めませんでした。"
    );

    let unwritable = scratch.join("no-such-directory/out.mp4");
    let mut job = CompositeExporter::new().start(&event, &unwritable, |_| {}, mtm);
    let state = run(&mut job, Duration::from_secs(60));
    println!("  unwritable output: {state:?}");
    let ExportState::Failed(error) = state else {
        panic!("{state:?}")
    };
    assert!(matches!(error, ExportError::RecordingFailed(_)));
    assert_eq!(
        (error.code(), error.message()),
        ("recording-failed", "動画の録画に失敗しました。")
    );
    assert_eq!(
        ExportError::Aborted.message(),
        "動画の書き出しをキャンセルしました。"
    );
}
