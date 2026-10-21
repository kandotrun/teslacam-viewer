//! Main-thread AVFoundation adapter that executes [`SyncMachine`] actions on a set of players.

use std::{
    collections::VecDeque,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use block2::RcBlock;
use objc2::{
    AnyThread, MainThreadMarker,
    rc::Retained,
    runtime::{AnyObject, Bool},
};
use objc2_av_foundation::{
    AVAsset, AVPlayer, AVPlayerActionAtItemEnd, AVPlayerItem, AVPlayerItemStatus,
    AVPlayerItemVideoOutput, AVPlayerStatus,
};
use objc2_core_foundation::CFRetained;
use objc2_core_media::{CMClock, kCMTimeInvalid, kCMTimeZero};
use objc2_core_video::{
    CVPixelBuffer, kCVPixelBufferIOSurfacePropertiesKey, kCVPixelBufferMetalCompatibilityKey,
    kCVPixelBufferPixelFormatTypeKey, kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString};

use crate::{
    av::{self, Inbox},
    frame::KEEPALIVE_FRAMES,
    status::{FileFault, classify_error},
    sync::{Action, Command, Notice, SyncMachine},
};

/// Loading state of one player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TileState {
    Loading,
    Ready,
    Failed(FileFault),
}

/// One `AVPlayer` + item + video output and the frames it handed out.
pub(crate) struct Tile {
    player: Retained<AVPlayer>,
    item: Retained<AVPlayerItem>,
    output: Retained<AVPlayerItemVideoOutput>,
    current: Option<CFRetained<CVPixelBuffer>>,
    keepalive: VecDeque<CFRetained<CVPixelBuffer>>,
    still_pending: bool,
    failure: Option<FileFault>,
}

fn video_output() -> Retained<AVPlayerItemVideoOutput> {
    let format = NSNumber::new_u32(kCVPixelFormatType_420YpCbCr8BiPlanarFullRange);
    let iosurface: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::new();
    let metal = NSNumber::new_bool(true);
    let attributes: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::from_slices(
        &[
            av::ns_key(unsafe { kCVPixelBufferPixelFormatTypeKey }),
            av::ns_key(unsafe { kCVPixelBufferIOSurfacePropertiesKey }),
            av::ns_key(unsafe { kCVPixelBufferMetalCompatibilityKey }),
        ],
        &[
            format.as_ref() as &AnyObject,
            iosurface.as_ref() as &AnyObject,
            metal.as_ref() as &AnyObject,
        ],
    );
    unsafe {
        AVPlayerItemVideoOutput::initWithPixelBufferAttributes(
            AVPlayerItemVideoOutput::alloc(),
            Some(&attributes),
        )
    }
}

impl Tile {
    pub fn new(asset: &AVAsset, clock: &CMClock, mtm: MainThreadMarker) -> Tile {
        let item = unsafe { AVPlayerItem::playerItemWithAsset(asset, mtm) };
        let output = video_output();
        unsafe { item.addOutput(&output) };
        let player = unsafe { AVPlayer::playerWithPlayerItem(Some(&item), mtm) };
        unsafe {
            // Required by setRate:time:atHostTime:, and keeps every player on the shared host clock.
            player.setAutomaticallyWaitsToMinimizeStalling(false);
            player.setActionAtItemEnd(AVPlayerActionAtItemEnd::Pause);
            player.setSourceClock(Some(clock));
        }
        Tile {
            player,
            item,
            output,
            current: None,
            keepalive: VecDeque::new(),
            still_pending: true,
            failure: None,
        }
    }

    pub fn state(&mut self) -> TileState {
        if let Some(fault) = self.failure {
            return TileState::Failed(fault);
        }
        let (item, player) = unsafe { (self.item.status(), self.player.status()) };
        if item == AVPlayerItemStatus::Failed || player == AVPlayerStatus::Failed {
            let error = unsafe { self.item.error().or_else(|| self.player.error()) };
            let fault = error.map_or(FileFault::Unavailable, |error| {
                classify_error(&av::media_error(&error))
            });
            self.failure = Some(fault);
            TileState::Failed(fault)
        } else if item == AVPlayerItemStatus::ReadyToPlay && player == AVPlayerStatus::ReadyToPlay {
            TileState::Ready
        } else {
            TileState::Loading
        }
    }

    fn is_ready(&mut self) -> bool {
        self.state() == TileState::Ready
    }

    fn show(&mut self, buffer: CFRetained<CVPixelBuffer>) {
        if let Some(previous) = self.current.replace(buffer) {
            self.keepalive.push_back(previous);
            while self.keepalive.len() > KEEPALIVE_FRAMES {
                self.keepalive.pop_front();
            }
        }
    }

    /// Pulls the frame due at host time `now` while running.
    fn pull_running(&mut self, now: f64) {
        let item_time = unsafe { self.output.itemTimeForHostTime(now) };
        if unsafe { self.output.hasNewPixelBufferForItemTime(item_time) }
            && let Some(buffer) = unsafe {
                self.output
                    .copyPixelBufferForItemTime_itemTimeForDisplay(item_time, std::ptr::null_mut())
            }
        {
            self.show(av::into_cf(buffer));
        }
    }

    /// Pulls the still frame at event time `t` once after a paused seek.
    fn pull_still(&mut self, t: f64) {
        if !self.still_pending {
            return;
        }
        if let Some(buffer) = unsafe {
            self.output
                .copyPixelBufferForItemTime_itemTimeForDisplay(av::cm(t), std::ptr::null_mut())
        } {
            self.show(av::into_cf(buffer));
            self.still_pending = false;
        }
    }

    pub fn current(&self) -> Option<&CFRetained<CVPixelBuffer>> {
        self.current.as_ref()
    }

    fn has_frame_for(&self, t: f64) -> bool {
        unsafe { self.output.hasNewPixelBufferForItemTime(av::cm(t)) }
    }
}

impl Drop for Tile {
    fn drop(&mut self) {
        unsafe {
            self.player.pause();
            self.item.removeOutput(&self.output);
            self.player.replaceCurrentItemWithPlayerItem(None);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Completion {
    Seek(u64),
    Preroll(u64),
}

/// How long the engine waits for slow players before starting with the ready ones. A player that
/// becomes ready later joins with a re-sync of every player.
const LOAD_GRACE_SECONDS: f64 = 5.0;

/// Runs a [`SyncMachine`] against real players. Main thread only.
pub(crate) struct Engine {
    tiles: Vec<Tile>,
    /// Which tiles were ready when the players were last brought in sync.
    joined: Vec<bool>,
    machine: SyncMachine,
    inbox: Inbox<Completion>,
    clock: CFRetained<CMClock>,
    duration: f64,
    attached_at: Option<f64>,
    loaded: bool,
}

impl Engine {
    pub fn new(machine: SyncMachine) -> Engine {
        Engine {
            tiles: Vec::new(),
            joined: Vec::new(),
            machine,
            inbox: Arc::new(Mutex::new(Vec::new())),
            clock: unsafe { CMClock::host_time_clock() },
            duration: 0.0,
            attached_at: None,
            loaded: false,
        }
    }

    /// The host time clock every player of this engine must use as its source clock.
    pub fn clock(&self) -> &CMClock {
        &self.clock
    }

    /// Installs the players once the media is composed; `duration` is the event duration.
    pub fn attach(&mut self, tiles: Vec<Tile>, duration: f64) {
        self.joined = vec![false; tiles.len()];
        self.tiles = tiles;
        self.duration = duration;
    }

    pub fn machine(&self) -> &SyncMachine {
        &self.machine
    }

    pub fn tile_state(&mut self, index: usize) -> TileState {
        self.tiles[index].state()
    }

    pub fn command(&mut self, command: Command, now: f64) {
        let actions = self.machine.command(command, now);
        self.execute(&actions);
    }

    /// Drains completions, detects readiness and advances the machine. `covered(tile, t)` tells
    /// whether a tile has video at event time `t` (tiles in a gap are not waited for).
    pub fn tick(&mut self, now: f64, covered: impl Fn(usize, f64) -> bool) -> Option<Notice> {
        let completions: Vec<Completion> = std::mem::take(&mut *self.inbox.lock().expect("inbox"));
        for completion in completions {
            let actions = match completion {
                Completion::Seek(generation) => self.machine.seek_done(generation, now),
                Completion::Preroll(generation) => self.machine.preroll_done(generation, now),
            };
            self.execute(&actions);
        }
        if !self.tiles.is_empty() {
            let attached_at = *self.attached_at.get_or_insert(now);
            let states: Vec<TileState> = self.tiles.iter_mut().map(Tile::state).collect();
            let any_ready = states.contains(&TileState::Ready);
            let late = states
                .iter()
                .zip(&self.joined)
                .any(|(state, joined)| *state == TileState::Ready && !joined);
            if !self.loaded {
                let settled = states.iter().all(|state| *state != TileState::Loading);
                if any_ready && (settled || now - attached_at >= LOAD_GRACE_SECONDS) {
                    self.loaded = true;
                    self.joined = states
                        .iter()
                        .map(|state| *state == TileState::Ready)
                        .collect();
                    let actions = self.machine.players_ready(self.duration, now);
                    self.execute(&actions);
                }
            } else if late {
                // A slow player became ready: bring everyone to the current position again.
                self.joined = states
                    .iter()
                    .map(|state| *state == TileState::Ready)
                    .collect();
                let position = self.machine.position(now);
                self.command(Command::Seek(position), now);
            }
        }
        let frames_ready = self.machine.awaiting_frames().is_some_and(|target| {
            self.tiles.iter_mut().enumerate().all(|(index, tile)| {
                !covered(index, target)
                    || tile.state() != TileState::Ready
                    || tile.has_frame_for(target)
            })
        });
        let (actions, notice) = self.machine.tick(now, frames_ready);
        self.execute(&actions);
        notice
    }

    /// Updates tile `index`'s frame for host time `now` (only while settled) and returns it.
    pub fn pull(&mut self, index: usize, now: f64) -> Option<&CFRetained<CVPixelBuffer>> {
        let running = self.machine.is_running();
        let settled = self.machine.is_settled();
        let position = self.machine.position(now);
        let tile = &mut self.tiles[index];
        if running {
            tile.pull_running(now);
        } else if settled {
            tile.pull_still(position);
        }
        tile.current()
    }

    fn execute(&mut self, actions: &[Action]) {
        for action in actions {
            match *action {
                Action::PauseAll => {
                    let host = unsafe { self.clock.time() };
                    for tile in self.ready_tiles() {
                        unsafe {
                            tile.player
                                .setRate_time_atHostTime(0.0, kCMTimeInvalid, host)
                        };
                    }
                }
                Action::SeekAll { target, generation } => {
                    let inbox = self.inbox.clone();
                    let tiles: Vec<&mut Tile> = self.ready_tiles().collect();
                    let pending = Arc::new(AtomicUsize::new(tiles.len()));
                    if tiles.is_empty() {
                        inbox
                            .lock()
                            .expect("inbox")
                            .push(Completion::Seek(generation));
                    }
                    for tile in tiles {
                        tile.still_pending = true;
                        let (pending, inbox) = (pending.clone(), inbox.clone());
                        let block = RcBlock::new(move |_finished: Bool| {
                            if pending.fetch_sub(1, Ordering::SeqCst) == 1 {
                                inbox
                                    .lock()
                                    .expect("inbox")
                                    .push(Completion::Seek(generation));
                            }
                        });
                        unsafe {
                            tile.player
                                .seekToTime_toleranceBefore_toleranceAfter_completionHandler(
                                    av::cm(target),
                                    kCMTimeZero,
                                    kCMTimeZero,
                                    &block,
                                )
                        };
                    }
                }
                Action::PrerollAll { rate, generation } => {
                    let inbox = self.inbox.clone();
                    let tiles: Vec<&mut Tile> = self.ready_tiles().collect();
                    let pending = Arc::new(AtomicUsize::new(tiles.len()));
                    if tiles.is_empty() {
                        inbox
                            .lock()
                            .expect("inbox")
                            .push(Completion::Preroll(generation));
                    }
                    for tile in tiles {
                        let (pending, inbox) = (pending.clone(), inbox.clone());
                        let block = RcBlock::new(move |_finished: Bool| {
                            if pending.fetch_sub(1, Ordering::SeqCst) == 1 {
                                inbox
                                    .lock()
                                    .expect("inbox")
                                    .push(Completion::Preroll(generation));
                            }
                        });
                        // prerollAtRate: requires a ready player at rate 0 (PauseAll ran first).
                        unsafe {
                            tile.player
                                .prerollAtRate_completionHandler(rate, Some(&block))
                        };
                    }
                }
                Action::StartAll {
                    rate,
                    position,
                    host_time,
                } => {
                    let (time, host) = (av::cm(position), av::host_cm(host_time));
                    for tile in self.ready_tiles() {
                        unsafe { tile.player.setRate_time_atHostTime(rate, time, host) };
                    }
                }
            }
        }
    }

    fn ready_tiles(&mut self) -> impl Iterator<Item = &mut Tile> {
        self.tiles
            .iter_mut()
            .filter_map(|tile| tile.is_ready().then_some(tile))
    }
}
