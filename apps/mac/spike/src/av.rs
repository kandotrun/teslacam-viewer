//! AVFoundation media-layer prototype.
//!
//! Threading: `AVPlayer` / `AVPlayerItem` are `MainThreadOnly` in objc2, so
//! everything here runs on the main thread. Completion handlers arrive on
//! arbitrary queues; the spike waits for them by pumping the main run loop
//! (`pump_until`). Inside GPUI the same results would be forwarded through a
//! oneshot channel and awaited from `cx.spawn`.

use std::{
    cell::RefCell,
    collections::VecDeque,
    path::{Path, PathBuf},
    ptr::NonNull,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use block2::RcBlock;
use objc2::{AnyThread, MainThreadMarker, rc::Retained, runtime::{AnyObject, Bool}};
use objc2_av_foundation::{
    AVAsset, AVAssetTrack, AVMediaTypeVideo, AVMutableComposition, AVPlayer,
    AVPlayerActionAtItemEnd, AVPlayerItem, AVPlayerItemStatus, AVPlayerItemVideoOutput,
    AVURLAsset, AVURLAssetPreferPreciseDurationAndTimingKey,
};
use objc2_core_foundation::{CFRetained, CFRunLoop, CFString, kCFRunLoopDefaultMode};
use objc2_core_media::{CMClock, CMTime, CMTimeRange, kCMTimeInvalid, kCMTimeZero};
use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetBaseAddressOfPlane, CVPixelBufferGetBytesPerRowOfPlane,
    CVPixelBufferGetHeightOfPlane, CVPixelBufferGetIOSurface, CVPixelBufferGetPixelFormatType,
    CVPixelBufferGetWidthOfPlane, CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
    CVPixelBufferUnlockBaseAddress, kCVPixelBufferIOSurfacePropertiesKey,
    kCVPixelBufferMetalCompatibilityKey, kCVPixelBufferPixelFormatTypeKey,
    kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
};
use objc2_foundation::{NSArray, NSDictionary, NSError, NSNumber, NSString, NSURL};
use objc2_quartz_core::CACurrentMediaTime;

use crate::counter::{self, Luma, Stamp};

/// What GPUI's `surface()` element takes in gpui-pre 0.3.9.
pub type PixelBuffer = CFRetained<CVPixelBuffer>;

/// 90 kHz is divisible by 36 fps, so frame boundaries are exact.
pub const TIMESCALE: i32 = 90_000;
pub const FPS: f64 = 36.0;
pub const SEGMENT_SECONDS: f64 = 60.0;

pub fn cm(seconds: f64) -> CMTime {
    unsafe { CMTime::with_seconds(seconds, TIMESCALE) }
}

pub fn secs(t: CMTime) -> f64 {
    unsafe { t.seconds() }
}

pub fn host_clock() -> CFRetained<CMClock> {
    unsafe { CMClock::host_time_clock() }
}

/// Host time as CMTime (mach_absolute_time domain), for `setRate:time:atHostTime:`.
pub fn host_now() -> CMTime {
    unsafe { host_clock().time() }
}

/// Host time in seconds (same domain as `itemTimeForHostTime:`).
pub fn media_now() -> f64 {
    CACurrentMediaTime()
}

/// Pump the current (main) run loop until `done` returns true or `timeout` elapses.
pub fn pump_until(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while !done() {
        if start.elapsed() > timeout {
            return false;
        }
        unsafe { CFRunLoop::run_in_mode(kCFRunLoopDefaultMode, 0.001, true) };
    }
    true
}

/// Pump the run loop for a fixed duration.
pub fn pump_for(duration: Duration) {
    let end = Instant::now() + duration;
    while Instant::now() < end {
        unsafe { CFRunLoop::run_in_mode(kCFRunLoopDefaultMode, 0.001, true) };
    }
}

fn cf_key(key: &CFString) -> &NSString {
    // CFString and NSString are toll-free bridged.
    unsafe { &*(key as *const CFString as *const NSString) }
}

pub fn file_url(path: &Path) -> Retained<NSURL> {
    NSURL::fileURLWithPath(&NSString::from_str(path.to_str().expect("utf-8 path")))
}

/// AVURLAsset with precise timing (needed for exact composition boundaries and seeks).
pub fn open_asset(path: &Path) -> Retained<AVURLAsset> {
    let key = unsafe { AVURLAssetPreferPreciseDurationAndTimingKey };
    let yes = NSNumber::new_bool(true);
    let options: Retained<NSDictionary<NSString, AnyObject>> =
        NSDictionary::from_slices(&[key], &[yes.as_ref() as &AnyObject]);
    unsafe { AVURLAsset::URLAssetWithURL_options(&file_url(path), Some(&options)) }
}

/// Loads the first video track with the async API and waits by pumping the run loop.
pub fn load_video_track(asset: &AVAsset) -> Result<Retained<AVAssetTrack>, String> {
    type Slot = Arc<Mutex<Option<Result<Retained<AVAssetTrack>, String>>>>;
    let slot: Slot = Arc::new(Mutex::new(None));
    let sink = slot.clone();
    let block = RcBlock::new(move |tracks: *mut NSArray<AVAssetTrack>, error: *mut NSError| {
        let result = if let Some(error) = unsafe { Retained::retain(error) } {
            Err(error.localizedDescription().to_string())
        } else {
            match unsafe { Retained::retain(tracks) } {
                Some(tracks) => tracks.firstObject().ok_or_else(|| "no video track".to_string()),
                None => Err("nil tracks".to_string()),
            }
        };
        *sink.lock().unwrap() = Some(result);
    });
    let media_type = unsafe { AVMediaTypeVideo }.expect("AVMediaTypeVideo");
    unsafe { asset.loadTracksWithMediaType_completionHandler(media_type, &block) };
    if !pump_until(Duration::from_secs(10), || slot.lock().unwrap().is_some()) {
        return Err("timed out loading tracks".into());
    }
    slot.lock().unwrap().take().unwrap()
}

/// One camera across an event: segment `i` occupies [i*60, (i+1)*60).
/// Missing segments (and short files) become empty edits so the timeline stays
/// aligned with the web app's fixed 60 s slots.
pub fn event_composition(
    segments: &[Option<PathBuf>],
) -> Result<Retained<AVMutableComposition>, String> {
    let composition = unsafe { AVMutableComposition::composition() };
    let media_type = unsafe { AVMediaTypeVideo }.expect("AVMediaTypeVideo");
    let track = unsafe { composition.addMutableTrackWithMediaType_preferredTrackID(media_type, 0) }
        .ok_or("addMutableTrack failed")?;
    for (index, segment) in segments.iter().enumerate() {
        let slot_start = index as f64 * SEGMENT_SECONDS;
        match segment {
            Some(path) => {
                let asset = open_asset(path);
                let source = load_video_track(&asset)?;
                let range = unsafe { source.timeRange() };
                let duration = secs(range.duration).min(SEGMENT_SECONDS);
                unsafe {
                    track
                        .insertTimeRange_ofTrack_atTime_error(
                            CMTimeRange::new(range.start, cm(duration)),
                            &source,
                            cm(slot_start),
                        )
                        .map_err(|e| e.localizedDescription().to_string())?;
                    if index == 0 {
                        track.setPreferredTransform(source.preferredTransform());
                    }
                }
                if duration < SEGMENT_SECONDS - 1e-3 {
                    unsafe {
                        track.insertEmptyTimeRange(CMTimeRange::new(
                            cm(slot_start + duration),
                            cm(SEGMENT_SECONDS - duration),
                        ))
                    };
                }
            }
            None => unsafe {
                track.insertEmptyTimeRange(CMTimeRange::new(cm(slot_start), cm(SEGMENT_SECONDS)))
            },
        }
    }
    Ok(composition)
}

/// Pixel buffer attributes: 420f (what gpui's Metal surface shader asserts),
/// IOSurface-backed and Metal compatible.
pub fn video_output() -> Retained<AVPlayerItemVideoOutput> {
    let format = NSNumber::new_u32(kCVPixelFormatType_420YpCbCr8BiPlanarFullRange);
    let iosurface: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::new();
    let metal = NSNumber::new_bool(true);
    let attributes: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::from_slices(
        &[
            cf_key(unsafe { kCVPixelBufferPixelFormatTypeKey }),
            cf_key(unsafe { kCVPixelBufferIOSurfacePropertiesKey }),
            cf_key(unsafe { kCVPixelBufferMetalCompatibilityKey }),
        ],
        &[format.as_ref() as &AnyObject, iosurface.as_ref() as &AnyObject, metal.as_ref() as &AnyObject],
    );
    unsafe {
        AVPlayerItemVideoOutput::initWithPixelBufferAttributes(
            AVPlayerItemVideoOutput::alloc(),
            Some(&attributes),
        )
    }
}

/// objc2-av-foundation hands out `Retained<CVPixelBuffer>` (+1, copy family);
/// gpui wants `CFRetained<CVPixelBuffer>`. Transfer the +1 without retain/release.
pub fn into_cf(buffer: Retained<CVPixelBuffer>) -> PixelBuffer {
    let raw = Retained::into_raw(buffer);
    unsafe { CFRetained::from_raw(NonNull::new_unchecked(raw)) }
}

pub fn is_iosurface_backed(buffer: &CVPixelBuffer) -> bool {
    CVPixelBufferGetIOSurface(Some(buffer)).is_some()
}

pub fn is_420f(buffer: &CVPixelBuffer) -> bool {
    CVPixelBufferGetPixelFormatType(buffer) == kCVPixelFormatType_420YpCbCr8BiPlanarFullRange
}

/// Reads the burned-in counter from the Y plane (CPU, read-only lock).
pub fn decode_stamp(buffer: &CVPixelBuffer) -> Result<Stamp, counter::DecodeError> {
    unsafe {
        CVPixelBufferLockBaseAddress(buffer, CVPixelBufferLockFlags::ReadOnly);
    }
    let base = CVPixelBufferGetBaseAddressOfPlane(buffer, 0) as *const u8;
    let stride = CVPixelBufferGetBytesPerRowOfPlane(buffer, 0);
    let width = CVPixelBufferGetWidthOfPlane(buffer, 0);
    let height = CVPixelBufferGetHeightOfPlane(buffer, 0);
    // Only the first 32 rows are needed.
    let rows = height.min(2 * counter::BLOCK);
    let result = if base.is_null() {
        Err(counter::DecodeError::PlaneTooSmall)
    } else {
        let data = unsafe { std::slice::from_raw_parts(base, stride * rows) };
        counter::decode(&Luma { data, stride, width, height: rows })
    };
    unsafe {
        CVPixelBufferUnlockBaseAddress(buffer, CVPixelBufferLockFlags::ReadOnly);
    }
    result
}

/// How many previously shown buffers to keep alive after they are replaced.
/// gpui's Metal renderer drops the CVMetalTexture right after encoding the draw,
/// so the IOSurface could be recycled by the output's pool while the GPU still
/// samples it. Holding the last few buffers prevents that.
pub const KEEPALIVE_FRAMES: usize = 3;

/// One player + item + video output, plus the frame currently on screen.
pub struct VideoTile {
    pub player: Retained<AVPlayer>,
    pub item: Retained<AVPlayerItem>,
    pub output: Retained<AVPlayerItemVideoOutput>,
    current: Option<PixelBuffer>,
    keepalive: VecDeque<PixelBuffer>,
    pub frames_pulled: u64,
}

impl VideoTile {
    pub fn new(asset: &AVAsset, mtm: MainThreadMarker) -> Self {
        let item = unsafe { AVPlayerItem::playerItemWithAsset(asset, mtm) };
        let output = video_output();
        unsafe { item.addOutput(&output) };
        let player = unsafe { AVPlayer::playerWithPlayerItem(Some(&item), mtm) };
        unsafe {
            player.setAutomaticallyWaitsToMinimizeStalling(false);
            player.setActionAtItemEnd(AVPlayerActionAtItemEnd::Pause);
        }
        Self { player, item, output, current: None, keepalive: VecDeque::new(), frames_pulled: 0 }
    }

    pub fn is_ready(&self) -> bool {
        unsafe { self.item.status() == AVPlayerItemStatus::ReadyToPlay }
    }

    pub fn item_time_for_host(&self, host_seconds: f64) -> f64 {
        secs(unsafe { self.output.itemTimeForHostTime(host_seconds) })
    }

    /// Pulls the frame due at `host_seconds` if the output has a new one.
    pub fn pull(&mut self, host_seconds: f64) -> bool {
        let item_time = unsafe { self.output.itemTimeForHostTime(host_seconds) };
        if !unsafe { self.output.hasNewPixelBufferForItemTime(item_time) } {
            return false;
        }
        let buffer = unsafe {
            self.output
                .copyPixelBufferForItemTime_itemTimeForDisplay(item_time, std::ptr::null_mut())
        };
        match buffer {
            Some(buffer) => {
                self.show(into_cf(buffer));
                true
            }
            None => false,
        }
    }

    /// Pulls whatever frame is due at the player's current item time (paused case).
    pub fn pull_current(&mut self) -> bool {
        let item_time = unsafe { self.item.currentTime() };
        let buffer = unsafe {
            self.output
                .copyPixelBufferForItemTime_itemTimeForDisplay(item_time, std::ptr::null_mut())
        };
        match buffer {
            Some(buffer) => {
                self.show(into_cf(buffer));
                true
            }
            None => false,
        }
    }

    fn show(&mut self, buffer: PixelBuffer) {
        if let Some(previous) = self.current.replace(buffer) {
            self.keepalive.push_back(previous);
            while self.keepalive.len() > KEEPALIVE_FRAMES {
                self.keepalive.pop_front();
            }
        }
        self.frames_pulled += 1;
    }

    pub fn current(&self) -> Option<&PixelBuffer> {
        self.current.as_ref()
    }

    pub fn clear(&mut self) {
        if let Some(previous) = self.current.take() {
            self.keepalive.push_back(previous);
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RestartReport {
    pub host_start: f64,
    pub seek: Duration,
    pub preroll: Duration,
    pub frames_wait: Duration,
    pub frames_ready: bool,
}

/// Shared-clock controller for N players.
pub struct SyncGroup {
    pub tiles: Vec<VideoTile>,
    clock: CFRetained<CMClock>,
}

impl SyncGroup {
    pub fn new(tiles: Vec<VideoTile>) -> Self {
        Self { tiles, clock: host_clock() }
    }

    /// All players run off the host time clock so one host time maps to the
    /// same item time everywhere.
    pub fn use_shared_host_clock(&self) {
        for tile in &self.tiles {
            unsafe { tile.player.setSourceClock(Some(&self.clock)) };
        }
    }

    pub fn wait_ready(&self, timeout: Duration) -> bool {
        pump_until(timeout, || self.tiles.iter().all(VideoTile::is_ready))
    }

    /// Start every player at the same host time. `item_time` None keeps the
    /// current position. Returns the host time (seconds) playback starts at.
    pub fn start_at(&self, rate: f32, item_time: Option<f64>, lead_seconds: f64) -> f64 {
        let host = unsafe { host_now().add(cm(lead_seconds)) };
        let time = item_time.map(cm).unwrap_or(unsafe { kCMTimeInvalid });
        for tile in &self.tiles {
            unsafe { tile.player.setRate_time_atHostTime(rate, time, host) };
        }
        secs(host)
    }

    pub fn pause(&self) {
        let host = host_now();
        for tile in &self.tiles {
            unsafe { tile.player.setRate_time_atHostTime(0.0, kCMTimeInvalid, host) };
        }
    }

    /// Zero-tolerance seek on every player; waits for all completion handlers.
    pub fn seek_exact(&self, seconds: f64, timeout: Duration) -> Result<Duration, String> {
        let started = Instant::now();
        let pending = Arc::new(AtomicUsize::new(self.tiles.len()));
        let finished_ok = Rc::new(RefCell::new(true));
        for tile in &self.tiles {
            let pending = pending.clone();
            let ok = finished_ok.clone();
            let block = RcBlock::new(move |finished: Bool| {
                if !finished.as_bool() {
                    *ok.borrow_mut() = false;
                }
                pending.fetch_sub(1, Ordering::SeqCst);
            });
            unsafe {
                tile.player.seekToTime_toleranceBefore_toleranceAfter_completionHandler(
                    cm(seconds),
                    kCMTimeZero,
                    kCMTimeZero,
                    &block,
                )
            };
        }
        if !pump_until(timeout, || pending.load(Ordering::SeqCst) == 0) {
            return Err("seek timed out".into());
        }
        if !*finished_ok.borrow() {
            return Err("a seek was interrupted".into());
        }
        Ok(started.elapsed())
    }

    /// Preroll every player at `rate` so the start is not limited by decode.
    pub fn preroll(&self, rate: f32, timeout: Duration) -> Result<Duration, String> {
        let started = Instant::now();
        let pending = Arc::new(AtomicUsize::new(self.tiles.len()));
        for tile in &self.tiles {
            let pending = pending.clone();
            let block = RcBlock::new(move |_finished: Bool| {
                pending.fetch_sub(1, Ordering::SeqCst);
            });
            unsafe { tile.player.prerollAtRate_completionHandler(rate, Some(&block)) };
        }
        if !pump_until(timeout, || pending.load(Ordering::SeqCst) == 0) {
            return Err("preroll timed out".into());
        }
        Ok(started.elapsed())
    }

    /// Async zero-tolerance seek for use inside GPUI (never pump the run loop
    /// from a GPUI callback: it re-enters the app). Resolves to true when every
    /// seek finished (not interrupted by a newer one).
    pub fn seek_exact_async(&self, seconds: f64) -> impl std::future::Future<Output = bool> + use<> {
        let receivers: Vec<_> = self
            .tiles
            .iter()
            .map(|tile| {
                let (tx, rx) = futures::channel::oneshot::channel::<bool>();
                let tx = Mutex::new(Some(tx));
                let block = RcBlock::new(move |finished: Bool| {
                    if let Some(tx) = tx.lock().unwrap().take() {
                        let _ = tx.send(finished.as_bool());
                    }
                });
                unsafe {
                    tile.player.seekToTime_toleranceBefore_toleranceAfter_completionHandler(
                        cm(seconds),
                        kCMTimeZero,
                        kCMTimeZero,
                        &block,
                    )
                };
                rx
            })
            .collect();
        async move {
            futures::future::join_all(receivers)
                .await
                .into_iter()
                .all(|r| r.unwrap_or(false))
        }
    }

    pub fn preroll_async(&self, rate: f32) -> impl std::future::Future<Output = bool> + use<> {
        let receivers: Vec<_> = self
            .tiles
            .iter()
            .map(|tile| {
                let (tx, rx) = futures::channel::oneshot::channel::<bool>();
                let tx = Mutex::new(Some(tx));
                let block = RcBlock::new(move |finished: Bool| {
                    if let Some(tx) = tx.lock().unwrap().take() {
                        let _ = tx.send(finished.as_bool());
                    }
                });
                unsafe { tile.player.prerollAtRate_completionHandler(rate, Some(&block)) };
                rx
            })
            .collect();
        async move {
            futures::future::join_all(receivers)
                .await
                .into_iter()
                .all(|r| r.unwrap_or(false))
        }
    }

    /// True when every output has a decoded (not yet acquired) frame for `seconds`.
    pub fn frames_ready_at(&self, seconds: f64) -> bool {
        self.tiles
            .iter()
            .all(|t| unsafe { t.output.hasNewPixelBufferForItemTime(cm(seconds)) })
    }

    /// Seek everything exactly, then restart all players on one host time.
    pub fn seek_and_restart(&self, seconds: f64, rate: f32, lead: f64) -> Result<f64, String> {
        let wait = std::env::var("SPIKE_WAIT_FRAMES").is_ok_and(|v| v == "1");
        self.seek_and_restart_ex(seconds, rate, lead, wait).map(|r| r.host_start)
    }

    /// Like `seek_and_restart`; with `wait_frames` it also waits (<= 300 ms) until
    /// every output can hand out the target frame before scheduling the start.
    pub fn seek_and_restart_ex(
        &self,
        seconds: f64,
        rate: f32,
        lead: f64,
        wait_frames: bool,
    ) -> Result<RestartReport, String> {
        self.pause();
        let seek = self.seek_exact(seconds, Duration::from_secs(5))?;
        if rate == 0.0 {
            return Ok(RestartReport { host_start: media_now(), seek, preroll: Duration::ZERO, frames_wait: Duration::ZERO, frames_ready: true });
        }
        let preroll = self.preroll(rate, Duration::from_secs(5))?;
        let t = Instant::now();
        let frames_ready = !wait_frames
            || pump_until(Duration::from_millis(300), || self.frames_ready_at(seconds));
        let frames_wait = t.elapsed();
        let host_start = self.start_at(rate, Some(seconds), lead);
        Ok(RestartReport { host_start, seek, preroll, frames_wait, frames_ready })
    }
}
