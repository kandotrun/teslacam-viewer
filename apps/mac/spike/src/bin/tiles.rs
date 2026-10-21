//! Q2 / Q3 / Q7 / Q8: GPUI Kit window with 6 synchronized AVFoundation tiles.
//!
//! Flags:
//!   --drive raf|poll   raf  = window.request_animation_frame() every render
//!                      poll = window.on_next_frame() polls the outputs and only
//!                             notifies when a tile has a new frame
//!   --start S --rate R --play-secs N --pause-secs N --verify --quit
use std::{path::PathBuf, time::Duration};

use gpui_kit::component::{
    ActiveTheme, h_flex,
    button::{Button, ButtonVariants},
    tag::Tag,
    v_flex,
};
use gpui_kit::*;
use teslacam_mac_spike::{
    av::{self, SyncGroup, VideoTile, media_now},
    fixtures::{self, CAMERA_LABELS},
};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Drive {
    Raf,
    Poll,
}

#[derive(Clone, Debug)]
struct Args {
    drive: Drive,
    start: f64,
    rate: f32,
    play_secs: f64,
    pause_secs: f64,
    verify: bool,
    quit: bool,
    throttle_inactive: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        drive: Drive::Poll,
        start: 50.0,
        rate: 1.0,
        play_secs: 20.0,
        pause_secs: 10.0,
        verify: false,
        quit: true,
        throttle_inactive: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "--drive" => {
                args.drive = match it.next().as_deref() {
                    Some("raf") => Drive::Raf,
                    _ => Drive::Poll,
                }
            }
            "--start" => args.start = it.next().unwrap().parse().unwrap(),
            "--rate" => args.rate = it.next().unwrap().parse().unwrap(),
            "--play-secs" => args.play_secs = it.next().unwrap().parse().unwrap(),
            "--pause-secs" => args.pause_secs = it.next().unwrap().parse().unwrap(),
            "--verify" => args.verify = true,
            "--stay" => args.quit = false,
            // Keep GPUI's default 30 fps cap for unfocused windows (for comparison).
            "--throttle-inactive" => args.throttle_inactive = true,
            other => panic!("unknown flag {other}"),
        }
    }
    args
}

static PROCESS_START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

/// CPU seconds (user + system) used by this process so far.
fn cpu_seconds() -> f64 {
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
    let tv = |t: libc::timeval| t.tv_sec as f64 + t.tv_usec as f64 / 1e6;
    tv(usage.ru_utime) + tv(usage.ru_stime)
}

#[derive(Default, Clone, Copy)]
struct Counters {
    renders: u64,
    frames_pulled: u64,
    ticks: u64,
    spread_hist: [u64; 4],
    decode_errors: u64,
}

struct Viewer {
    group: SyncGroup,
    drive: Drive,
    playing: bool,
    rate: f32,
    verify: bool,
    counters: Counters,
    folder: Option<PathBuf>,
    status: SharedString,
}

/// TeslaCam tile order of the web theater grid (top row then bottom row).
const GRID: [[usize; 3]; 2] = [[2, 0, 4], [3, 1, 5]];

impl Viewer {
    fn new(cx: &mut Context<Self>, args: &Args) -> Self {
        let mtm = objc2::MainThreadMarker::new().expect("GPUI runs on the main thread");
        let tiles = (0..6)
            .map(|camera| {
                let composition = av::event_composition(&fixtures::camera_segments(camera))
                    .expect("composition");
                VideoTile::new(&composition, mtm)
            })
            .collect();
        let group = SyncGroup::new(tiles);
        group.use_shared_host_clock();
        let _ = cx;
        Self {
            group,
            drive: args.drive,
            playing: false,
            rate: args.rate,
            verify: args.verify,
            counters: Counters::default(),
            folder: None,
            status: "準備中".into(),
        }
    }

    fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        println!("REFRESH interval={:?} scale={}", window.refresh_interval(cx), window.scale_factor());
        self.group.start_at(self.rate, None, 0.05);
        self.playing = true;
        self.status = "再生中".into();
        if self.drive == Drive::Poll {
            self.schedule_poll(window, cx);
        }
        cx.notify();
    }

    fn pause(&mut self, cx: &mut Context<Self>) {
        self.group.pause();
        self.playing = false;
        self.status = "一時停止".into();
        cx.notify();
    }

    fn pull_all(&mut self) -> bool {
        let now = media_now();
        let mut any = false;
        for tile in &mut self.group.tiles {
            if tile.pull(now) {
                self.counters.frames_pulled += 1;
                any = true;
            }
        }
        any
    }

    /// Display-link driven poll: runs once per display frame while playing, but
    /// only invalidates the view when at least one tile got a new frame.
    fn schedule_poll(&self, window: &mut Window, cx: &mut Context<Self>) {
        let this = cx.weak_entity();
        window.on_next_frame(move |window, cx| {
            let _ = this.update(cx, |viewer, cx| {
                if !viewer.playing {
                    return;
                }
                viewer.counters.ticks += 1;
                if viewer.pull_all() {
                    cx.notify();
                }
                viewer.schedule_poll(window, cx);
            });
        });
    }

    fn pick_folder(&mut self, cx: &mut Context<Self>) {
        // Q2: directory picker API shape (NSOpenPanel). Not clicked in automation.
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("フォルダを選択".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(mut paths))) = receiver.await {
                let folder = paths.pop();
                let _ = this.update(cx, |viewer, cx| {
                    viewer.folder = folder;
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn verify_spread(&mut self) {
        let frames: Vec<i64> = self
            .group
            .tiles
            .iter()
            .filter_map(|t| t.current().and_then(|b| av::decode_stamp(b).ok()))
            .map(|s| s.frame as i64)
            .collect();
        if frames.len() == 6 {
            let spread = frames.iter().max().unwrap() - frames.iter().min().unwrap();
            self.counters.spread_hist[(spread as usize).min(3)] += 1;
        } else {
            self.counters.decode_errors += 1;
        }
    }

    fn tile(&self, camera: usize, cx: &Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let content = match self.group.tiles[camera].current() {
            Some(buffer) => div()
                .size_full()
                .child(surface(buffer.clone()).size_full().object_fit(ObjectFit::Contain)),
            None => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(theme.muted_foreground)
                .child("記録なし"),
        };
        div()
            .relative()
            .flex_1()
            .h_full()
            .bg(gpui_kit::black())
            .rounded_md()
            .overflow_hidden()
            .child(content)
            .child(
                div()
                    .absolute()
                    .top_2()
                    .left_2()
                    .px_2()
                    .py_0p5()
                    .rounded_sm()
                    .bg(hsla(0., 0., 0., 0.72))
                    .text_color(gpui_kit::white())
                    .text_sm()
                    .child(CAMERA_LABELS[camera]),
            )
    }
}

impl Render for Viewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.counters.renders += 1;
        if self.counters.renders == 1 {
            let since_start = PROCESS_START.get().map(|t| t.elapsed().as_secs_f64() * 1000.0);
            println!("FIRST_RENDER since_process_start_ms={since_start:?}");
        }
        if self.playing && self.drive == Drive::Raf {
            self.pull_all();
            window.request_animation_frame();
        }
        if self.verify && self.playing {
            self.verify_spread();
        }
        let theme = cx.theme().clone();
        let elapsed = self.group.tiles[0].item_time_for_host(media_now()).max(0.0);
        let rates = [0.5f32, 1.0, 2.0, 4.0];
        v_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .p_3()
            .gap_3()
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().text_lg().child("TeslaCam ビューアー"))
                    .child(Tag::secondary().child(self.status.clone()))
                    .child(div().flex_1())
                    .child(
                        Button::new("pick-folder")
                            .outline()
                            .label("フォルダを選択")
                            .on_click(cx.listener(|this, _, _, cx| this.pick_folder(cx))),
                    )
                    .child(
                        Button::new("play-pause")
                            .primary()
                            .label(if self.playing { "一時停止" } else { "再生" })
                            .on_click(cx.listener(|this, _, window, cx| {
                                if this.playing {
                                    this.pause(cx)
                                } else {
                                    this.start(window, cx)
                                }
                            })),
                    )
                    .children(rates.iter().map(|&rate| {
                        let button = Button::new(SharedString::from(format!("rate-{rate}")))
                            .label(format!("{rate}x"))
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.rate = rate;
                                if this.playing {
                                    this.start(window, cx);
                                }
                            }));
                        if (self.rate - rate).abs() < f32::EPSILON { button.primary() } else { button.ghost() }
                    })),
            )
            .child(
                v_flex().flex_1().gap_2().children(GRID.iter().map(|row| {
                    h_flex().flex_1().gap_2().children(row.iter().map(|&camera| self.tile(camera, cx)))
                })),
            )
            .child(
                h_flex()
                    .gap_3()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child(format!("{:02}:{:05.2} / 02:00", (elapsed / 60.0) as u32, elapsed % 60.0))
                    .child(format!(
                        "描画 {} 回 / 取得フレーム {}",
                        self.counters.renders, self.counters.frames_pulled
                    ))
                    .child(match &self.folder {
                        Some(folder) => format!("選択中: {}", folder.display()),
                        None => "フォルダ未選択".to_string(),
                    }),
            )
    }
}

fn report(label: &str, before: (f64, f64, Counters), after: (f64, f64, Counters)) {
    let wall = after.0 - before.0;
    let cpu = after.1 - before.1;
    let c = |f: fn(&Counters) -> u64| f(&after.2) - f(&before.2);
    let spread_total: u64 = (0..4).map(|i| after.2.spread_hist[i] - before.2.spread_hist[i]).sum();
    let pct = |i: usize| {
        100.0 * (after.2.spread_hist[i] - before.2.spread_hist[i]) as f64 / spread_total.max(1) as f64
    };
    println!(
        "PHASE {label} wall={wall:.2}s cpu={cpu:.2}s cpu_pct={:.1}% renders={} renders_per_s={:.1} frames_pulled={} frames_per_s={:.1} ticks={} verify_samples={spread_total} spread0={:.1}% spread1={:.1}% spread2={:.1}% spread3+={:.1}% decode_err={}",
        100.0 * cpu / wall,
        c(|c| c.renders),
        c(|c| c.renders) as f64 / wall,
        c(|c| c.frames_pulled),
        c(|c| c.frames_pulled) as f64 / wall,
        c(|c| c.ticks),
        pct(0),
        pct(1),
        pct(2),
        pct(3),
        c(|c| c.decode_errors),
    );
}

fn main() {
    PROCESS_START.get_or_init(std::time::Instant::now);
    let args = parse_args();
    println!("ARGS {args:?} pid={}", std::process::id());
    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);
        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        let (window, viewer) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("TeslaCam Spike".into()),
                    ..Default::default()
                }),
                // GPUI caps unfocused windows to ~30 fps (33.3 ms) by default;
                // 36 fps dashcam video needs the cap off.
                inactive_frame_interval: if args.throttle_inactive {
                    Some(Duration::from_micros(33_333))
                } else {
                    None
                },
                ..Default::default()
            },
            cx,
            |_, cx| cx.new(|cx| Viewer::new(cx, &args)),
        )
        .expect("open window");
        cx.activate(true);
        let args = args.clone();
        let viewer = viewer.downgrade();
        cx.spawn(async move |cx| {
            let executor = cx.background_executor().clone();
            let timer = |secs: f64| executor.timer(Duration::from_secs_f64(secs));
            // Wait until all players are ready.
            loop {
                let ready = viewer
                    .update(cx, |v, _| v.group.tiles.iter().all(VideoTile::is_ready))
                    .unwrap_or(false);
                if ready {
                    break;
                }
                timer(0.02).await;
            }
            // Exact seek + preroll, then a synchronized start.
            let seek = viewer.update(cx, |v, _| v.group.seek_exact_async(args.start)).unwrap();
            println!("SEEK ok={}", seek.await);
            let preroll = viewer.update(cx, |v, _| v.group.preroll_async(args.rate)).unwrap();
            println!("PREROLL ok={}", preroll.await);
            // Hold the start until every output has the target frame decoded.
            for _ in 0..30 {
                if viewer.update(cx, |v, _| v.group.frames_ready_at(args.start)).unwrap_or(true) {
                    break;
                }
                timer(0.01).await;
            }
            // Show the paused target frame on every tile before starting.
            viewer
                .update(cx, |v, cx| {
                    for tile in &mut v.group.tiles {
                        tile.pull_current();
                    }
                    cx.notify();
                })
                .unwrap();
            let snapshot = |cx: &mut AsyncApp| {
                viewer.update(cx, |v, _| (media_now(), cpu_seconds(), v.counters)).unwrap()
            };
            let before_idle = snapshot(cx);
            timer(3.0).await;
            report("idle_before_play", before_idle, snapshot(cx));
            window
                .update(cx, |_, window, cx| {
                    viewer.update(cx, |v, cx| v.start(window, cx)).unwrap();
                })
                .unwrap();
            println!("PLAYING");
            timer(1.0).await;
            let before = snapshot(cx);
            timer(args.play_secs).await;
            report("play", before, snapshot(cx));
            viewer.update(cx, |v, cx| v.pause(cx)).unwrap();
            println!("PAUSED");
            timer(1.0).await;
            let before = snapshot(cx);
            timer(args.pause_secs).await;
            report("paused", before, snapshot(cx));
            if args.quit {
                cx.update(|cx| cx.quit());
            }
        })
        .detach();
    });
}
