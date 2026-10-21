//! Synchronized playback, statuses and the representative player against real media.

use std::time::{Duration, Instant};

use objc2::MainThreadMarker;
use teslacam_media::{
    CameraId, CameraMap, CameraStatus, EventInput, EventPlayback, FileFault, PlaybackEvent,
    ProbeJob, RepresentativePlayer, SegmentInput, host_now, run_loop::pump_until,
};

use crate::{
    counter::{Stamp, decode_buffer},
    fixtures::{self, frame_at},
};

pub const TESTS: &[crate::Test] = &[
    (
        "playback_loads_the_full_event",
        playback_loads_the_full_event,
    ),
    (
        "paused_seeks_show_the_exact_frame_on_every_camera",
        paused_seeks_show_the_exact_frame_on_every_camera,
    ),
    (
        "sync_spread_is_zero_across_the_segment_boundary_at_every_rate",
        sync_spread_is_zero_across_the_segment_boundary_at_every_rate,
    ),
    (
        "sync_spread_is_zero_after_seeks_while_playing",
        sync_spread_is_zero_after_seeks_while_playing,
    ),
    ("rate_changes_restart_in_sync", rate_changes_restart_in_sync),
    (
        "playback_ends_and_pauses_at_the_end",
        playback_ends_and_pauses_at_the_end,
    ),
    (
        "gaps_show_missing_and_keep_the_other_cameras_in_sync",
        gaps_show_missing_and_keep_the_other_cameras_in_sync,
    ),
    (
        "faulty_files_map_to_window_states",
        faulty_files_map_to_window_states,
    ),
    (
        "commands_before_loading_are_applied_once_ready",
        commands_before_loading_are_applied_once_ready,
    ),
    (
        "representative_player_plays_event_mp4",
        representative_player_plays_event_mp4,
    ),
    (
        "probe_job_reports_durations_sizes_and_faults",
        probe_job_reports_durations_sizes_and_faults,
    ),
    (
        "short_tails_hold_the_last_frame",
        short_tails_hold_the_last_frame,
    ),
];

const LOAD_TIMEOUT: Duration = Duration::from_secs(15);

/// Ticks until loading finished and every camera that has video at the current time is ready.
fn open_ready(event: &EventInput, mtm: MainThreadMarker) -> EventPlayback {
    let mut playback = EventPlayback::open(event, mtm);
    let mut loaded = false;
    let ready = pump_until(LOAD_TIMEOUT, || {
        let now = host_now();
        loaded |= playback
            .tick(now)
            .iter()
            .any(|e| matches!(e, PlaybackEvent::Loaded { .. }));
        loaded
            && playback.is_settled()
            && CameraId::ALL
                .iter()
                .all(|c| playback.status(*c, now) != CameraStatus::Loading)
    });
    assert!(ready, "event did not load");
    playback
}

/// Ticks until no seek or start is in flight.
fn settle(playback: &mut EventPlayback) {
    let ok = pump_until(Duration::from_secs(5), || {
        playback.tick(host_now());
        playback.is_settled()
    });
    assert!(ok, "playback did not settle");
}

fn stamps(playback: &mut EventPlayback, cameras: &[CameraId], now: f64) -> Vec<Option<Stamp>> {
    cameras
        .iter()
        .map(|c| {
            playback
                .frame(*c, now)
                .and_then(|f| decode_buffer(&f.buffer))
        })
        .collect()
}

#[derive(Debug, Default)]
struct SyncStats {
    ticks: usize,
    complete: usize,
    spread_max: u32,
    lag_max: i64,
    min_frame: u32,
    max_frame: u32,
    wrong_camera: usize,
}

/// Emulates a 120 Hz render loop while running and measures the spread of the frames on screen.
fn sample(playback: &mut EventPlayback, cameras: &[CameraId], duration: Duration) -> SyncStats {
    let mut stats = SyncStats {
        min_frame: u32::MAX,
        ..Default::default()
    };
    let start = Instant::now();
    let mut next = start;
    while start.elapsed() < duration {
        pump_until(Duration::from_secs(1), || Instant::now() >= next);
        next += Duration::from_secs_f64(1.0 / 120.0);
        let now = host_now();
        playback.tick(now);
        if !playback.is_running() {
            continue;
        }
        stats.ticks += 1;
        let shown = stamps(playback, cameras, now);
        for (camera, stamp) in cameras.iter().zip(&shown) {
            if stamp.is_some_and(|s| s.id as usize != camera.index() + 1) {
                stats.wrong_camera += 1;
            }
        }
        let frames: Vec<u32> = shown.iter().flatten().map(|s| s.frame).collect();
        if frames.len() == cameras.len() {
            stats.complete += 1;
            let (min, max) = (*frames.iter().min().unwrap(), *frames.iter().max().unwrap());
            stats.spread_max = stats.spread_max.max(max - min);
            stats.min_frame = stats.min_frame.min(min);
            stats.max_frame = stats.max_frame.max(max);
            let expected = i64::from(frame_at(playback.current_time(now)));
            stats.lag_max = stats.lag_max.max((expected - i64::from(min)).abs());
        }
    }
    stats
}

fn play_from(playback: &mut EventPlayback, t: f64) {
    playback.seek(t, host_now());
    playback.play(host_now());
    let ok = pump_until(Duration::from_secs(5), || {
        playback.tick(host_now());
        playback.is_running()
    });
    assert!(ok, "playback did not start");
}

fn playback_loads_the_full_event(mtm: MainThreadMarker) {
    let mut playback = open_ready(&fixtures::full_event(), mtm);
    let duration = playback.duration().expect("loaded");
    assert!((duration - 120.0).abs() < 1e-3, "duration {duration}");
    let now = host_now();
    for camera in CameraId::ALL {
        assert_eq!(
            playback.status(camera, now),
            CameraStatus::Ready,
            "{camera:?}"
        );
    }
    let probe = playback.probe().expect("probe");
    assert_eq!(probe.len(), 2);
    let front = probe[1].cameras[CameraId::Front]
        .expect("file")
        .expect("playable");
    assert!((front.duration - 60.0).abs() < 1e-3);
    assert_eq!((front.display_width, front.display_height), (1280.0, 960.0));
    assert!(!playback.is_playing());
    println!("  loaded: duration {duration:.3} s");
}

fn paused_seeks_show_the_exact_frame_on_every_camera(mtm: MainThreadMarker) {
    let mut playback = open_ready(&fixtures::full_event(), mtm);
    for target in [0.0, 30.0, 59.95, 60.0, 61.5, 119.9] {
        playback.seek(target, host_now());
        settle(&mut playback);
        let want = frame_at(target);
        let mut shown = Vec::new();
        let ok = pump_until(Duration::from_secs(3), || {
            shown = stamps(&mut playback, &CameraId::ALL, host_now());
            shown.iter().all(|s| s.is_some_and(|s| s.frame == want))
        });
        assert!(ok, "seek {target}: want frame {want}, shown {shown:?}");
        for (camera, stamp) in CameraId::ALL.iter().zip(&shown) {
            assert_eq!(stamp.unwrap().id as usize, camera.index() + 1);
        }
    }
}

fn sync_spread_is_zero_across_the_segment_boundary_at_every_rate(mtm: MainThreadMarker) {
    let mut playback = open_ready(&fixtures::full_event(), mtm);
    for rate in [0.5f32, 1.0, 2.0, 4.0] {
        playback.pause(host_now());
        settle(&mut playback);
        playback.set_rate(rate, host_now());
        // Cross the 60 s boundary about 1.5 s into the window.
        play_from(&mut playback, 60.0 - 1.5 * f64::from(rate));
        let stats = sample(&mut playback, &CameraId::ALL, Duration::from_secs(3));
        println!("  rate {rate}: {stats:?}");
        assert!(stats.complete > 200, "too few complete ticks: {stats:?}");
        assert_eq!(stats.spread_max, 0, "rate {rate}: {stats:?}");
        assert_eq!(stats.wrong_camera, 0);
        assert!(
            stats.max_frame > 2160 + 18,
            "did not cross the boundary: {stats:?}"
        );
        assert!(stats.lag_max <= 2, "frames lag the clock: {stats:?}");
    }
    playback.pause(host_now());
}

fn sync_spread_is_zero_after_seeks_while_playing(mtm: MainThreadMarker) {
    let mut playback = open_ready(&fixtures::full_event(), mtm);
    play_from(&mut playback, 1.0);
    for target in [5.0, 33.3, 59.9, 60.0, 61.2, 90.5, 118.0, 12.34] {
        let started = Instant::now();
        playback.seek(target, host_now());
        let ok = pump_until(Duration::from_secs(5), || {
            playback.tick(host_now());
            playback.is_running()
        });
        assert!(ok, "did not restart after seeking to {target}");
        let restart_ms = started.elapsed().as_secs_f64() * 1000.0;
        let stats = sample(&mut playback, &CameraId::ALL, Duration::from_millis(700));
        println!("  seek {target}: restart {restart_ms:.0} ms {stats:?}");
        assert_eq!(stats.spread_max, 0, "seek {target}: {stats:?}");
        assert!(
            stats.min_frame >= frame_at(target),
            "stale frames after seeking to {target}: {stats:?}"
        );
    }
    playback.pause(host_now());
}

fn rate_changes_restart_in_sync(mtm: MainThreadMarker) {
    let mut playback = open_ready(&fixtures::full_event(), mtm);
    play_from(&mut playback, 10.0);
    sample(&mut playback, &CameraId::ALL, Duration::from_millis(500));
    playback.set_rate(2.0, host_now());
    let before = playback.current_time(host_now());
    pump_until(Duration::from_secs(5), || {
        playback.tick(host_now());
        playback.is_running()
    });
    let started = Instant::now();
    let stats = sample(&mut playback, &CameraId::ALL, Duration::from_secs(1));
    let advanced = playback.current_time(host_now()) - before;
    let wall = started.elapsed().as_secs_f64();
    println!("  2x: advanced {advanced:.2} s in {wall:.2} s wall; {stats:?}");
    assert_eq!(stats.spread_max, 0);
    assert!(advanced > 1.7 * wall, "rate not applied");
    assert_eq!(playback.rate(), 2.0);
    playback.pause(host_now());
}

fn playback_ends_and_pauses_at_the_end(mtm: MainThreadMarker) {
    let mut playback = open_ready(&fixtures::full_event(), mtm);
    play_from(&mut playback, 119.0);
    let mut ended = false;
    let ok = pump_until(Duration::from_secs(4), || {
        ended |= playback.tick(host_now()).contains(&PlaybackEvent::Ended);
        ended
    });
    assert!(ok, "no Ended event");
    let now = host_now();
    assert!(!playback.is_playing());
    assert!((playback.current_time(now) - 120.0).abs() < 1e-6);
    // Play after the end restarts from the beginning.
    playback.play(host_now());
    let ok = pump_until(Duration::from_secs(5), || {
        playback.tick(host_now());
        playback.is_running()
    });
    assert!(ok);
    assert!(playback.current_time(host_now()) < 1.0);
    playback.pause(host_now());
}

fn gaps_show_missing_and_keep_the_other_cameras_in_sync(mtm: MainThreadMarker) {
    let mut playback = open_ready(&fixtures::gaps_event(), mtm);
    let duration = playback.duration().unwrap();
    assert!((duration - 60.0).abs() < 1e-3, "duration {duration}");
    let expectations: [(f64, &[(CameraId, CameraStatus)]); 3] = [
        (
            5.0,
            &[
                (CameraId::Front, CameraStatus::Ready),
                (CameraId::Back, CameraStatus::Ready),
                (CameraId::RightRepeater, CameraStatus::Missing),
            ],
        ),
        (
            25.0,
            &[
                (CameraId::Front, CameraStatus::Ready),
                (CameraId::Back, CameraStatus::Missing),
                (CameraId::LeftPillar, CameraStatus::Ready),
            ],
        ),
        (
            30.0,
            &[
                (CameraId::Front, CameraStatus::Missing),
                (CameraId::Back, CameraStatus::Missing),
                (CameraId::RightPillar, CameraStatus::Ready),
            ],
        ),
    ];
    for (t, statuses) in expectations {
        playback.seek(t, host_now());
        settle(&mut playback);
        for (camera, status) in statuses {
            assert_eq!(
                playback.status(*camera, host_now()),
                *status,
                "{camera:?} at {t}"
            );
            if *status == CameraStatus::Missing {
                assert!(
                    playback.frame(*camera, host_now()).is_none(),
                    "{camera:?} at {t} has a frame"
                );
            }
        }
    }
    // Front's short clip: frames at 25 s come from the 8 s clip of segment 1.
    playback.seek(25.0, host_now());
    settle(&mut playback);
    let want = frame_at(25.0);
    let ok = pump_until(Duration::from_secs(2), || {
        stamps(
            &mut playback,
            &[CameraId::Front, CameraId::LeftRepeater],
            host_now(),
        ) == [
            Some(Stamp { frame: want, id: 1 }),
            Some(Stamp { frame: want, id: 4 }),
        ]
    });
    assert!(ok, "front short clip frame");
    // Across the 20 s boundary (where the back clip ends) the remaining cameras stay in sync.
    let cameras = [
        CameraId::Front,
        CameraId::LeftPillar,
        CameraId::LeftRepeater,
        CameraId::RightPillar,
    ];
    play_from(&mut playback, 18.5);
    let stats = sample(&mut playback, &cameras, Duration::from_secs(3));
    println!("  across 20 s: {stats:?}");
    assert_eq!(stats.spread_max, 0, "{stats:?}");
    assert!(stats.max_frame > frame_at(20.5));
    assert_eq!(
        playback.status(CameraId::Back, host_now()),
        CameraStatus::Missing
    );
    // At 40 s the back clip returns and the front gap ends: both rejoin in sync while playing.
    let cameras = [CameraId::Front, CameraId::Back, CameraId::LeftPillar];
    play_from(&mut playback, 38.5);
    let stats = sample(&mut playback, &cameras, Duration::from_secs(3));
    println!("  across 40 s: {stats:?}");
    assert!(
        stats.complete > 100,
        "back / front did not come back: {stats:?}"
    );
    assert_eq!(stats.spread_max, 0, "{stats:?}");
    assert!(stats.min_frame >= frame_at(40.0));
    assert_eq!(
        playback.status(CameraId::Back, host_now()),
        CameraStatus::Ready
    );
    playback.pause(host_now());
}

fn faulty_files_map_to_window_states(mtm: MainThreadMarker) {
    let scratch = fixtures::scratch();
    let garbage = scratch.join("garbage.mp4");
    std::fs::write(&garbage, b"\0\x01\x02\x03").unwrap();
    let source = fixtures::clip(&fixtures::full_dir(), "2026-10-20_12-00-00", CameraId::Back);
    let bytes = std::fs::read(&source).unwrap();
    let truncated = scratch.join("truncated.mp4");
    std::fs::write(&truncated, &bytes[..bytes.len() / 3]).unwrap();
    let text = scratch.join("text.mp4");
    std::fs::write(&text, "this is not a movie\n".repeat(200)).unwrap();
    let good = fixtures::clip(
        &fixtures::full_dir(),
        "2026-10-20_12-00-00",
        CameraId::RightPillar,
    );
    let event = EventInput {
        segments: vec![SegmentInput {
            start_seconds: 0.0,
            files: CameraMap::from_fn(|camera| match camera {
                CameraId::Front => Some(garbage.clone()),
                CameraId::Back => Some(truncated.clone()),
                CameraId::LeftPillar => Some(scratch.join("does-not-exist.mp4")),
                CameraId::LeftRepeater => Some(text.clone()),
                CameraId::RightPillar => Some(good.clone()),
                CameraId::RightRepeater => None,
            }),
        }],
    };
    let mut playback = open_ready(&event, mtm);
    let now = host_now();
    let statuses = CameraMap::from_fn(|camera| playback.status(camera, now));
    println!("  statuses: {statuses:?}");
    assert!(matches!(
        statuses[CameraId::Front],
        CameraStatus::Corrupt | CameraStatus::Unsupported
    ));
    assert!(matches!(
        statuses[CameraId::LeftRepeater],
        CameraStatus::Corrupt | CameraStatus::Unsupported
    ));
    assert_eq!(statuses[CameraId::Back], CameraStatus::Corrupt);
    assert_eq!(statuses[CameraId::LeftPillar], CameraStatus::Unavailable);
    assert_eq!(statuses[CameraId::RightPillar], CameraStatus::Ready);
    assert_eq!(statuses[CameraId::RightRepeater], CameraStatus::Missing);
    assert!((playback.duration().unwrap() - 60.0).abs() < 1e-3);

    // An event in which nothing can be played loads with duration 0 and terminal statuses.
    let broken = EventInput {
        segments: vec![SegmentInput {
            start_seconds: 0.0,
            files: CameraMap::from_fn(|c| (c == CameraId::Front).then(|| garbage.clone())),
        }],
    };
    let mut playback = EventPlayback::open(&broken, mtm);
    let mut duration = None;
    assert!(pump_until(LOAD_TIMEOUT, || {
        for event in playback.tick(host_now()) {
            if let PlaybackEvent::Loaded { duration: d } = event {
                duration = Some(d);
            }
        }
        duration.is_some()
    }));
    assert_eq!(duration, Some(0.0));
    assert!(playback.status(CameraId::Front, host_now()).is_terminal());
    assert_eq!(
        playback.status(CameraId::Back, host_now()),
        CameraStatus::Missing
    );
}

fn commands_before_loading_are_applied_once_ready(mtm: MainThreadMarker) {
    let mut playback = EventPlayback::open(&fixtures::full_event(), mtm);
    let now = host_now();
    assert_eq!(playback.status(CameraId::Front, now), CameraStatus::Loading);
    playback.seek(45.0, now);
    playback.play(now);
    let ok = pump_until(LOAD_TIMEOUT, || {
        playback.tick(host_now());
        playback.is_running()
    });
    assert!(ok, "autoplay after loading");
    let stats = sample(&mut playback, &CameraId::ALL, Duration::from_millis(500));
    assert_eq!(stats.spread_max, 0);
    assert!(
        stats.min_frame >= frame_at(45.0) && stats.max_frame < frame_at(47.0),
        "{stats:?}"
    );
    playback.pause(host_now());
}

fn representative_player_plays_event_mp4(mtm: MainThreadMarker) {
    let mut player = RepresentativePlayer::open(&fixtures::representative(), mtm);
    let ok = pump_until(LOAD_TIMEOUT, || {
        player.tick(host_now());
        player.status(host_now()) == CameraStatus::Ready && player.is_settled()
    });
    assert!(ok, "representative did not load");
    assert!((player.duration().unwrap() - 20.0).abs() < 1e-3);
    player.seek(12.5, host_now());
    let ok = pump_until(Duration::from_secs(3), || {
        player.tick(host_now());
        player.is_settled()
            && player
                .frame(host_now())
                .and_then(|f| decode_buffer(&f.buffer))
                == Some(Stamp {
                    frame: frame_at(12.5),
                    id: 7,
                })
    });
    assert!(ok, "representative paused seek");
    let frame = player.frame(host_now()).unwrap();
    assert_eq!((frame.display_width, frame.display_height), (1280.0, 720.0));
    player.play(host_now());
    // tick() applies the seek/preroll completions, so keep calling it like a render loop would.
    let started = Instant::now();
    pump_until(Duration::from_secs(1), || {
        player.tick(host_now());
        started.elapsed() > Duration::from_millis(600)
    });
    let t = player.current_time(host_now());
    assert!(t > 12.6, "representative did not advance: {t}");
    player.pause(host_now());

    let mut missing =
        RepresentativePlayer::open(&fixtures::scratch().join("missing-event.mp4"), mtm);
    assert!(pump_until(LOAD_TIMEOUT, || {
        missing.tick(host_now());
        missing.status(host_now()) != CameraStatus::Loading
    }));
    assert_eq!(missing.status(host_now()), CameraStatus::Unavailable);
}

fn probe_job_reports_durations_sizes_and_faults(_mtm: MainThreadMarker) {
    let mut event = fixtures::gaps_event();
    event.segments[0].files[CameraId::RightRepeater] =
        Some(fixtures::scratch().join("not-there.mp4"));
    let mut job = ProbeJob::start(&event);
    assert!(pump_until(LOAD_TIMEOUT, || job.try_result().is_some()));
    let segments = job.try_result().unwrap();
    assert_eq!(segments.len(), 3);
    assert_eq!(segments[1].start_seconds, 20.0);
    let front = segments[1].cameras[CameraId::Front].unwrap().unwrap();
    assert!((front.duration - 8.0).abs() < 1e-3, "{front:?}");
    assert_eq!((front.display_width, front.display_height), (1280.0, 960.0));
    assert!((front.frame_duration - 1.0 / 36.0).abs() < 1e-6);
    assert_eq!(segments[1].cameras[CameraId::Back], None);
    assert_eq!(
        segments[0].cameras[CameraId::RightRepeater],
        Some(Err(FileFault::Unavailable))
    );
}

fn short_tails_hold_the_last_frame(mtm: MainThreadMarker) {
    // A 19.6 s copy of the first front clip, followed by the next segment at 20 s: the 0.4 s tail
    // holds the last frame instead of flashing a missing tile.
    let source = fixtures::clip(
        &fixtures::gaps_dir(),
        "2026-10-20_13-00-00",
        CameraId::Front,
    );
    let trimmed = fixtures::scratch().join("front-19.6s.mp4");
    let status = std::process::Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(&source)
        .args(["-t", "19.6", "-c", "copy"])
        .arg(&trimmed)
        .status()
        .expect("ffmpeg");
    assert!(status.success());
    let mut event = fixtures::gaps_event();
    event.segments[0].files[CameraId::Front] = Some(trimmed);
    let mut playback = open_ready(&event, mtm);
    let clip = playback.timeline().unwrap().cameras[CameraId::Front].clips[0];
    assert!(clip.hold > 0.3 && clip.hold < 0.45, "{clip:?}");
    let last = (clip.duration * fixtures::FPS).round() as u32 - 1;
    for t in [clip.duration + 0.05, 19.95] {
        playback.seek(t, host_now());
        settle(&mut playback);
        assert_eq!(
            playback.status(CameraId::Front, host_now()),
            CameraStatus::Ready
        );
        let ok = pump_until(Duration::from_secs(2), || {
            stamps(&mut playback, &[CameraId::Front], host_now())
                == [Some(Stamp { frame: last, id: 1 })]
        });
        assert!(ok, "t {t}: front does not hold frame {last}");
    }
    // Playing through the tail: front holds its last frame until 20 s, then both cameras run in
    // sync again from the first frame of segment 1.
    let cameras = [CameraId::Front, CameraId::LeftPillar];
    play_from(&mut playback, 19.0);
    let during = sample(&mut playback, &cameras, Duration::from_millis(1100));
    let after = sample(&mut playback, &cameras, Duration::from_secs(1));
    println!("  through the held tail: {during:?}");
    println!("  after the held tail: {after:?}");
    assert!(
        during.spread_max <= (clip.hold * fixtures::FPS).ceil() as u32 + 1,
        "{during:?}"
    );
    assert!(after.min_frame >= frame_at(20.0), "{after:?}");
    assert_eq!(after.spread_max, 0, "{after:?}");
    playback.pause(host_now());
}
