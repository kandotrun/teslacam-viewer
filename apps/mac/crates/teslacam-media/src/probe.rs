//! Probing clip files: duration, display size and whether a file can be shown at all.
//!
//! Probing runs on a worker thread (AVFoundation asset loading may block on I/O). Only immutable
//! `AVURLAsset` / `AVAssetTrack` objects cross back to the caller.

use std::{
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use objc2::rc::Retained;
use objc2_av_foundation::{AVAssetTrack, AVURLAsset};

use crate::{
    av::{self, SendBox},
    input::{CameraMap, EventInput},
    layout,
    status::{FileFault, classify_error},
    timeline::{ClipInfo, ClipProbe, SegmentProbe},
};

/// How long one file may take to load its tracks before it counts as unavailable.
const LOAD_TIMEOUT: Duration = Duration::from_secs(15);

/// One probed file plus the AVFoundation objects needed to compose it.
pub(crate) struct ProbedFile {
    pub probe: ClipProbe,
    pub media: Option<(Retained<AVURLAsset>, Retained<AVAssetTrack>)>,
}

/// Probe results of an event, segment by segment, with the loaded assets.
pub(crate) struct ProbedEvent {
    pub segments: Vec<SegmentProbe>,
    pub files: Vec<CameraMap<Option<ProbedFile>>>,
}

impl ProbedEvent {
    /// The loaded video track of `camera` in `segment`, if the file can be shown.
    pub fn track(&self, segment: usize, camera: crate::input::CameraId) -> Option<&AVAssetTrack> {
        self.files
            .get(segment)?
            .get(camera)
            .as_ref()?
            .media
            .as_ref()
            .map(|(_, track)| &**track)
    }
}

/// Probes every file of `event` on the calling thread (which must not be the main thread, because it
/// waits for AVFoundation completion handlers). Returns `None` when `cancel` was set.
pub(crate) fn probe_event_blocking(event: &EventInput, cancel: &AtomicBool) -> Option<ProbedEvent> {
    // Start every load first so the files load in parallel, then collect them in order.
    type Pending = Option<Result<(Retained<AVURLAsset>, PendingTracks), FileFault>>;
    type PendingTracks =
        mpsc::Receiver<Result<Option<SendBox<Retained<AVAssetTrack>>>, crate::status::MediaError>>;
    let mut pending: Vec<CameraMap<Pending>> = Vec::with_capacity(event.segments.len());
    for segment in &event.segments {
        if cancel.load(Ordering::SeqCst) {
            return None;
        }
        pending.push(segment.files.map(|_, file| {
            file.as_deref().map(|path| {
                check_file(path)?;
                let asset = av::open_asset(path);
                let rx = av::load_video_tracks(&asset);
                Ok((asset, rx))
            })
        }));
    }
    let mut files = Vec::with_capacity(pending.len());
    for (segment, cameras) in event.segments.iter().zip(pending) {
        let mut probed = CameraMap::from_fn(|_| None);
        for (camera, entry) in cameras {
            if cancel.load(Ordering::SeqCst) {
                return None;
            }
            let Some(entry) = entry else { continue };
            let path = segment.files[camera]
                .as_deref()
                .expect("pending entries have a path");
            probed[camera] = Some(match entry {
                Err(fault) => ProbedFile {
                    probe: Err(fault),
                    media: None,
                },
                Ok((asset, rx)) => finish_probe(path, asset, &rx),
            });
        }
        files.push(probed);
    }
    let segments = event
        .segments
        .iter()
        .zip(&files)
        .map(|(segment, files)| SegmentProbe {
            start_seconds: segment.start_seconds,
            cameras: files.map(|_, file| file.as_ref().map(|file| file.probe)),
        })
        .collect();
    Some(ProbedEvent { segments, files })
}

/// Probes one file on the calling thread (not the main thread).
pub(crate) fn probe_file_blocking(path: &Path) -> ProbedFile {
    if let Err(fault) = check_file(path) {
        return ProbedFile {
            probe: Err(fault),
            media: None,
        };
    }
    let asset = av::open_asset(path);
    let rx = av::load_video_tracks(&asset);
    finish_probe(path, asset, &rx)
}

/// File-system checks before AVFoundation sees the file.
fn check_file(path: &Path) -> Result<(), FileFault> {
    let metadata = std::fs::metadata(path).map_err(|_| FileFault::Unavailable)?;
    if !metadata.is_file() {
        return Err(FileFault::Unavailable);
    }
    if metadata.len() == 0 {
        return Err(FileFault::Corrupt);
    }
    Ok(())
}

fn finish_probe(
    path: &Path,
    asset: Retained<AVURLAsset>,
    rx: &mpsc::Receiver<Result<Option<SendBox<Retained<AVAssetTrack>>>, crate::status::MediaError>>,
) -> ProbedFile {
    let failed = |fault| ProbedFile {
        probe: Err(fault),
        media: None,
    };
    let track = match av::wait(rx, LOAD_TIMEOUT) {
        None => return failed(FileFault::Unavailable),
        Some(Err(error)) => return failed(classify_error(&error)),
        Some(Ok(None)) => return failed(FileFault::Unsupported),
        Some(Ok(Some(SendBox(track)))) => track,
    };
    // The tracks are loaded, so these reads do not block for long (and this is a worker thread).
    let (time_range, natural, preferred, decodable, sample_bytes, frame_rate) = unsafe {
        (
            track.timeRange(),
            track.naturalSize(),
            track.preferredTransform(),
            track.isDecodable(),
            track.totalSampleDataLength(),
            track.nominalFrameRate(),
        )
    };
    if !decodable {
        return failed(FileFault::Unsupported);
    }
    let duration = av::secs(time_range.duration);
    if !duration.is_finite() || duration <= 0.0 {
        return failed(FileFault::Corrupt);
    }
    let file_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if sample_bytes > 0 && file_bytes < sample_bytes as u64 {
        // The sample table references more data than the file holds: a truncated copy.
        return failed(FileFault::Corrupt);
    }
    let (display_width, display_height) =
        layout::display_size((natural.width, natural.height), av::affine_parts(preferred));
    let frame_duration = if frame_rate > 1.0 {
        1.0 / f64::from(frame_rate)
    } else {
        1.0 / 30.0
    };
    let info = ClipInfo {
        source_start: av::secs(time_range.start),
        duration,
        display_width,
        display_height,
        frame_duration,
    };
    ProbedFile {
        probe: Ok(info),
        media: Some((asset, track)),
    }
}

/// A background probe of an event's files (durations, display sizes, decodability).
///
/// Create it with [`ProbeJob::start`]; poll [`ProbeJob::try_result`] from any thread (e.g. once per
/// frame on the main thread) or block with [`ProbeJob::wait`] off the main thread.
pub struct ProbeJob {
    rx: mpsc::Receiver<Vec<SegmentProbe>>,
    result: Option<Vec<SegmentProbe>>,
}

impl ProbeJob {
    /// Starts probing on a worker thread.
    pub fn start(event: &EventInput) -> ProbeJob {
        let (tx, rx) = mpsc::channel();
        let event = event.clone();
        thread::spawn(move || {
            let cancel = AtomicBool::new(false);
            if let Some(probed) = probe_event_blocking(&event, &cancel) {
                let _ = tx.send(probed.segments);
            }
        });
        ProbeJob { rx, result: None }
    }

    /// The per-segment probe results once probing finished.
    pub fn try_result(&mut self) -> Option<&[SegmentProbe]> {
        if self.result.is_none() {
            self.result = self.rx.try_recv().ok();
        }
        self.result.as_deref()
    }

    /// Blocks until probing finished. Do not call on the main thread inside an app: completion
    /// handlers may need the main run loop.
    pub fn wait(mut self) -> Vec<SegmentProbe> {
        match self.result.take() {
            Some(result) => result,
            None => self.rx.recv().unwrap_or_default(),
        }
    }
}
