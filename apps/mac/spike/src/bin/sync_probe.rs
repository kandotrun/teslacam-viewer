//! Q4 / Q5 probe: 6 AVPlayers on event compositions, measured by decoding the
//! burned-in frame counter of the pixel buffer each tile would show.
//!
//! Usage: sync_probe [sync|naive|event|all]
use std::time::{Duration, Instant};

use objc2::MainThreadMarker;
use objc2_core_media::kCMTimeInvalid;
use teslacam_mac_spike::{
    av::{self, FPS, SyncGroup, VideoTile, cm, media_now, pump_until},
    fixtures,
};

#[derive(Default, Debug)]
struct Stats {
    ticks: usize,
    complete_ticks: usize,
    spread_hist: [usize; 4], // 0, 1, 2, >=3
    spread_max: u32,
    item_time_spread_max_ms: f64,
    lag_sum: i64,
    lag_min: i64,
    lag_max: i64,
    decode_errors: usize,
    camera_mismatch: usize,
    new_frames: u64,
}

impl Stats {
    fn line(&self, label: &str) -> String {
        let n = self.complete_ticks.max(1) as f64;
        format!(
            "RESULT {label} ticks={} complete={} spread_max={} spread0={:.1}% spread1={:.1}% spread2={:.1}% spread3+={:.1}% itemtime_spread_max_ms={:.3} lag_mean={:.2} lag_min={} lag_max={} decode_err={} cam_mismatch={} new_frames={}",
            self.ticks,
            self.complete_ticks,
            self.spread_max,
            100.0 * self.spread_hist[0] as f64 / n,
            100.0 * self.spread_hist[1] as f64 / n,
            100.0 * self.spread_hist[2] as f64 / n,
            100.0 * self.spread_hist[3] as f64 / n,
            self.item_time_spread_max_ms,
            self.lag_sum as f64 / n,
            self.lag_min,
            self.lag_max,
            self.decode_errors,
            self.camera_mismatch,
            self.new_frames,
        )
    }
}

/// Emulates a display-rate render loop (`hz`): pull, then decode what is on screen.
fn sample(group: &mut SyncGroup, duration: Duration, hz: f64) -> Stats {
    let mut stats = Stats { lag_min: i64::MAX, lag_max: i64::MIN, ..Default::default() };
    let start = Instant::now();
    let period = Duration::from_secs_f64(1.0 / hz);
    let mut next = start;
    while start.elapsed() < duration {
        pump_until(Duration::from_secs(1), || Instant::now() >= next);
        next += period;
        let now = media_now();
        stats.ticks += 1;
        let mut frames = Vec::with_capacity(group.tiles.len());
        let mut item_times = Vec::with_capacity(group.tiles.len());
        for (index, tile) in group.tiles.iter_mut().enumerate() {
            if tile.pull(now) {
                stats.new_frames += 1;
            }
            item_times.push(tile.item_time_for_host(now));
            if let Some(buffer) = tile.current() {
                match av::decode_stamp(buffer) {
                    Ok(stamp) => {
                        if stamp.camera as usize != index {
                            stats.camera_mismatch += 1;
                        }
                        frames.push(stamp.frame as i64);
                    }
                    Err(_) => stats.decode_errors += 1,
                }
            }
        }
        let it_min = item_times.iter().cloned().fold(f64::INFINITY, f64::min);
        let it_max = item_times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        stats.item_time_spread_max_ms = stats.item_time_spread_max_ms.max((it_max - it_min) * 1000.0);
        if frames.len() == group.tiles.len() {
            stats.complete_ticks += 1;
            let min = *frames.iter().min().unwrap();
            let max = *frames.iter().max().unwrap();
            let spread = (max - min) as u32;
            stats.spread_max = stats.spread_max.max(spread);
            stats.spread_hist[(spread as usize).min(3)] += 1;
            // Lag: frame due at the shared item time minus the frame shown.
            let expected = (item_times[0] * FPS + 1e-6).floor() as i64;
            let lag = expected - min;
            stats.lag_sum += lag;
            stats.lag_min = stats.lag_min.min(lag);
            stats.lag_max = stats.lag_max.max(lag);
        }
    }
    stats
}

fn build_group(mtm: MainThreadMarker, naive: bool) -> SyncGroup {
    let tiles = (0..6)
        .map(|camera| {
            let composition = av::event_composition(&fixtures::camera_segments(camera))
                .expect("composition");
            let tile = VideoTile::new(&composition, mtm);
            if naive {
                // Typical default: let AVPlayer decide when to start.
                unsafe { tile.player.setAutomaticallyWaitsToMinimizeStalling(true) };
            }
            tile
        })
        .collect();
    let group = SyncGroup::new(tiles);
    assert!(group.wait_ready(Duration::from_secs(10)), "players not ready");
    group
}

fn naive_seek_and_play(group: &SyncGroup, seconds: f64, rate: f32) {
    for tile in &group.tiles {
        unsafe {
            tile.player.pause();
            tile.player.seekToTime(cm(seconds));
        }
    }
    for tile in &group.tiles {
        unsafe { tile.player.setRate(rate) };
    }
}

fn run_rates(group: &mut SyncGroup, naive: bool) {
    let mode = if naive { "naive" } else { "sync" };
    for rate in [0.5f32, 1.0, 2.0, 4.0] {
        // Start so that the 60 s segment boundary is crossed ~2 s into the window.
        let start = 60.0 - 2.0 * rate as f64;
        let t0 = Instant::now();
        if naive {
            naive_seek_and_play(group, start, rate);
        } else {
            group.seek_and_restart(start, rate, 0.05).expect("seek_and_restart");
        }
        let setup_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let after = sample(group, Duration::from_millis(1000), 120.0);
        println!("{} setup_ms={setup_ms:.1}", after.line(&format!("mode={mode} rate={rate} phase=after_seek_1s")));
        let steady = sample(group, Duration::from_millis(4000), 120.0);
        println!("{}", steady.line(&format!("mode={mode} rate={rate} phase=steady_4s")));
    }
}

fn run_seeks(group: &mut SyncGroup, naive: bool) {
    let mode = if naive { "naive" } else { "sync" };
    let mut worst = 0;
    for target in [5.0, 33.3, 59.9, 60.0, 61.2, 90.5, 118.0, 12.34] {
        let t0 = Instant::now();
        if naive {
            naive_seek_and_play(group, target, 1.0);
        } else {
            let wait = std::env::var("SPIKE_WAIT_FRAMES").is_ok_and(|v| v == "1");
            let r = group.seek_and_restart_ex(target, 1.0, 0.05, wait).expect("seek");
            println!(
                "RESTART seek_ms={:.1} preroll_ms={:.1} frames_wait_ms={:.1} frames_ready={}",
                r.seek.as_secs_f64() * 1000.0,
                r.preroll.as_secs_f64() * 1000.0,
                r.frames_wait.as_secs_f64() * 1000.0,
                r.frames_ready
            );
        }
        let setup_ms = t0.elapsed().as_secs_f64() * 1000.0;
        // Time until every tile shows a frame >= the seek target frame.
        let wanted = (target * FPS + 1e-6).floor() as i64;
        let first_sync = Instant::now();
        let mut synced_after_ms = f64::NAN;
        let s = sample_until_synced(group, wanted, &mut synced_after_ms, first_sync);
        worst = worst.max(s.spread_max);
        println!(
            "{} setup_ms={setup_ms:.1} all_tiles_on_target_after_ms={synced_after_ms:.1}",
            s.line(&format!("mode={mode} seek_to={target} phase=after_seek_1s"))
        );
    }
    println!("RESULT mode={mode} seeks worst_spread_after_seek={worst}");
}

fn sample_until_synced(
    group: &mut SyncGroup,
    wanted: i64,
    synced_after_ms: &mut f64,
    start: Instant,
) -> Stats {
    // Same as `sample`, but also records when all tiles reached the target frame.
    let mut total = Stats { lag_min: i64::MAX, lag_max: i64::MIN, ..Default::default() };
    while start.elapsed() < Duration::from_millis(1000) {
        let s = sample(group, Duration::from_millis(8), 120.0);
        let all_on_target = group.tiles.iter().all(|t| {
            t.current()
                .and_then(|b| av::decode_stamp(b).ok())
                .is_some_and(|stamp| stamp.frame as i64 >= wanted)
        });
        if all_on_target && synced_after_ms.is_nan() {
            *synced_after_ms = start.elapsed().as_secs_f64() * 1000.0;
        }
        merge(&mut total, &s);
    }
    total
}

fn merge(total: &mut Stats, s: &Stats) {
    total.ticks += s.ticks;
    total.complete_ticks += s.complete_ticks;
    for i in 0..4 {
        total.spread_hist[i] += s.spread_hist[i];
    }
    total.spread_max = total.spread_max.max(s.spread_max);
    total.item_time_spread_max_ms = total.item_time_spread_max_ms.max(s.item_time_spread_max_ms);
    total.lag_sum += s.lag_sum;
    if s.complete_ticks > 0 {
        total.lag_min = total.lag_min.min(s.lag_min);
        total.lag_max = total.lag_max.max(s.lag_max);
    }
    total.decode_errors += s.decode_errors;
    total.camera_mismatch += s.camera_mismatch;
    total.new_frames += s.new_frames;
}

fn run_long_steady(group: &mut SyncGroup, naive: bool) {
    let mode = if naive { "naive" } else { "sync" };
    if naive {
        naive_seek_and_play(group, 2.0, 1.0);
    } else {
        group.seek_and_restart(2.0, 1.0, 0.05).unwrap();
    }
    pump_until(Duration::from_millis(500), || false);
    let s = sample(group, Duration::from_secs(20), 120.0);
    println!("{}", s.line(&format!("mode={mode} rate=1 phase=steady_20s")));
}

/// Q5: event-track behaviour on a single camera.
fn run_event(mtm: MainThreadMarker) {
    // Exact paused seeks across the boundary.
    let composition = av::event_composition(&fixtures::camera_segments(0)).unwrap();
    println!(
        "EVENT composition duration={:.4}s",
        av::secs(unsafe { objc2_av_foundation::AVAsset::duration(&composition) })
    );
    let mut group = SyncGroup::new(vec![VideoTile::new(&composition, mtm)]);
    group.use_shared_host_clock();
    assert!(group.wait_ready(Duration::from_secs(10)));
    for target in [0.0, 30.0, 59.95, 59.98, 60.0, 60.01, 61.5, 119.95] {
        group.seek_exact(target, Duration::from_secs(5)).unwrap();
        let tile = &mut group.tiles[0];
        let got = pump_until(Duration::from_secs(2), || tile.pull_current());
        let frame = tile.current().and_then(|b| av::decode_stamp(b).ok()).map(|s| s.frame as i64);
        let expected = (target * FPS + 1e-6).floor() as i64;
        println!(
            "EVENT paused_seek target={target} expected_frame={expected} shown_frame={frame:?} ok={} got_buffer={got}",
            frame == Some(expected)
        );
    }
    // Continuous playback across the boundary, sampling at 240 Hz.
    let start = 58.0;
    group.seek_and_restart(start, 1.0, 0.05).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    let mut boundary_wall: Option<f64> = None;
    let mut last_new = Instant::now();
    let mut max_gap_ms: f64 = 0.0;
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_secs(4) {
        pump_until(Duration::from_millis(5), || false);
        let tile = &mut group.tiles[0];
        if tile.pull(media_now()) {
            max_gap_ms = max_gap_ms.max(last_new.elapsed().as_secs_f64() * 1000.0);
            last_new = Instant::now();
            if let Some(stamp) = tile.current().and_then(|b| av::decode_stamp(b).ok()) {
                seen.insert(stamp.frame);
                if stamp.frame >= 2160 && boundary_wall.is_none() {
                    boundary_wall = Some(t0.elapsed().as_secs_f64());
                }
            }
        }
    }
    let lo = (start * FPS) as u32 + 2;
    let hi = *seen.iter().max().unwrap_or(&0);
    let missing: Vec<u32> = (lo..=hi).filter(|f| !seen.contains(f)).collect();
    println!(
        "EVENT continuous start={start} frames_seen={} range={lo}..={hi} missing={} first_missing={:?} crossed_boundary_at_wall={boundary_wall:?} max_gap_between_new_frames_ms={max_gap_ms:.1}",
        seen.len(),
        missing.len(),
        missing.iter().take(10).collect::<Vec<_>>()
    );
    group.pause();

    // Missing segments as empty edits.
    for (label, segments) in [
        ("missing_second", vec![Some(fixtures::clip(0, 1)), None]),
        ("missing_first", vec![None, Some(fixtures::clip(1, 1))]),
    ] {
        let composition = av::event_composition(&segments).unwrap();
        let duration = av::secs(unsafe { objc2_av_foundation::AVAsset::duration(&composition) });
        let mut group = SyncGroup::new(vec![VideoTile::new(&composition, mtm)]);
        group.use_shared_host_clock();
        assert!(group.wait_ready(Duration::from_secs(10)));
        for target in [30.0, 90.0] {
            group.seek_exact(target, Duration::from_secs(5)).unwrap();
            group.tiles[0].clear();
            let tile = &mut group.tiles[0];
            let got = pump_until(Duration::from_millis(500), || tile.pull_current());
            let frame = tile.current().and_then(|b| av::decode_stamp(b).ok()).map(|s| s.frame);
            println!("EVENT {label} duration={duration:.3} paused_seek={target} buffer={got} frame={frame:?}");
        }
        // Play across the boundary and record the first/last frames seen.
        group.seek_and_restart(58.5, 1.0, 0.05).unwrap();
        group.tiles[0].clear();
        let mut first: Option<(f64, u32)> = None;
        let mut last: Option<(f64, u32)> = None;
        let mut pulls_with_new = 0;
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_secs(3) {
            pump_until(Duration::from_millis(5), || false);
            let now = media_now();
            let item_time = group.tiles[0].item_time_for_host(now);
            if group.tiles[0].pull(now) {
                pulls_with_new += 1;
                if let Some(stamp) = group.tiles[0].current().and_then(|b| av::decode_stamp(b).ok()) {
                    if first.is_none() {
                        first = Some((item_time, stamp.frame));
                    }
                    last = Some((item_time, stamp.frame));
                }
            }
        }
        let rate = unsafe { group.tiles[0].player.rate() };
        println!(
            "EVENT {label} play_58.5_to_61.5 first(item_time,frame)={first:?} last={last:?} new_frames={pulls_with_new} rate_after={rate}"
        );
        group.pause();
    }
    let _ = unsafe { kCMTimeInvalid };
}

fn main() {
    let mtm = MainThreadMarker::new().expect("main thread");
    let what = std::env::args().nth(1).unwrap_or_else(|| "all".into());
    if what == "seeks" {
        let mut group = build_group(mtm, false);
        group.use_shared_host_clock();
        run_seeks(&mut group, false);
        run_rates(&mut group, false);
        group.pause();
    }
    if what == "sync" || what == "all" {
        let mut group = build_group(mtm, false);
        group.use_shared_host_clock();
        let probe = &group.tiles[0];
        let start = Instant::now();
        pump_until(Duration::from_secs(2), || false);
        let _ = (probe, start);
        run_rates(&mut group, false);
        run_seeks(&mut group, false);
        run_long_steady(&mut group, false);
        // Report buffer properties once.
        if let Some(buffer) = group.tiles[0].current() {
            println!(
                "BUFFER 420f={} iosurface={} size={}x{}",
                av::is_420f(buffer),
                av::is_iosurface_backed(buffer),
                objc2_core_video::CVPixelBufferGetWidth(buffer),
                objc2_core_video::CVPixelBufferGetHeight(buffer)
            );
        }
        group.pause();
    }
    if what == "naive" || what == "all" {
        let mut group = build_group(mtm, true);
        run_rates(&mut group, true);
        run_seeks(&mut group, true);
        run_long_steady(&mut group, true);
        for tile in &group.tiles {
            unsafe { tile.player.pause() };
        }
    }
    if what == "event" || what == "all" {
        run_event(mtm);
    }
}
