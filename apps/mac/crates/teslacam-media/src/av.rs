//! Small AVFoundation / CoreMedia helpers shared by the playback and export code.

use std::{path::Path, ptr::NonNull, sync::mpsc, time::Duration};

use block2::RcBlock;
use objc2::{rc::Retained, runtime::AnyObject};
use objc2_av_foundation::{
    AVAssetTrack, AVMediaTypeVideo, AVMutableComposition, AVMutableCompositionTrack, AVURLAsset,
    AVURLAssetPreferPreciseDurationAndTimingKey,
};
use objc2_core_foundation::{CFRetained, CFString, CGAffineTransform};
use objc2_core_media::{CMTime, CMTimeRange};
use objc2_core_video::CVPixelBuffer;
use objc2_foundation::{NSArray, NSDictionary, NSError, NSNumber, NSString, NSURL};

use crate::{
    status::MediaError,
    timeline::{CameraTimeline, PlacedClip},
};

/// 90 kHz divides 30 and 36 fps exactly, so frame boundaries stay exact on the event axis.
pub(crate) const TIMESCALE: i32 = 90_000;

/// Event / media seconds as a `CMTime`.
pub(crate) fn cm(seconds: f64) -> CMTime {
    unsafe { CMTime::with_seconds(seconds, TIMESCALE) }
}

/// Host seconds (`CACurrentMediaTime` domain) as a host-clock `CMTime`.
pub(crate) fn host_cm(seconds: f64) -> CMTime {
    unsafe { CMTime::with_seconds(seconds, 1_000_000_000) }
}

/// A `CMTime` in seconds.
pub(crate) fn secs(time: CMTime) -> f64 {
    unsafe { time.seconds() }
}

pub(crate) fn range(start: f64, duration: f64) -> CMTimeRange {
    CMTimeRange {
        start: cm(start),
        duration: cm(duration),
    }
}

/// Moves a value to another thread.
///
/// Used only for AVFoundation objects that Apple documents as safe to hand between threads when they
/// are not used concurrently: immutable assets and tracks (`AVURLAsset`, `AVAssetTrack`) and a
/// composition that the sending thread has finished building and never touches again.
pub(crate) struct SendBox<T>(pub T);

// SAFETY: see the type docs; every use hands the value over exactly once and keeps no other reference.
unsafe impl<T> Send for SendBox<T> {}

pub(crate) fn file_url(path: &Path) -> Retained<NSURL> {
    NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
}

fn cf_key(key: &CFString) -> &NSString {
    // SAFETY: CFString and NSString are toll-free bridged.
    unsafe { &*(key as *const CFString as *const NSString) }
}

pub(crate) fn ns_key(key: &CFString) -> &NSString {
    cf_key(key)
}

/// An `AVURLAsset` with precise timing (exact composition boundaries and exact seeks).
pub(crate) fn open_asset(path: &Path) -> Retained<AVURLAsset> {
    let key = unsafe { AVURLAssetPreferPreciseDurationAndTimingKey };
    let yes = NSNumber::new_bool(true);
    let options: Retained<NSDictionary<NSString, AnyObject>> =
        NSDictionary::from_slices(&[key], &[yes.as_ref() as &AnyObject]);
    unsafe { AVURLAsset::URLAssetWithURL_options(&file_url(path), Some(&options)) }
}

/// Copies an `NSError` (and its underlying errors) into plain data.
pub(crate) fn media_error(error: &NSError) -> MediaError {
    let mut copied = MediaError::new(
        error.domain().to_string(),
        error.code() as i64,
        error.localizedDescription().to_string(),
    );
    if let Some(underlying) = error.underlyingErrors().firstObject() {
        copied = copied.with_underlying(media_error(&underlying));
    }
    copied
}

/// A media error that did not come from an `NSError`.
pub(crate) fn local_error(description: impl Into<String>) -> MediaError {
    MediaError::new("TeslaCamMedia", -1, description)
}

/// Starts loading the video tracks of `asset`; the result arrives on the returned channel from an
/// AVFoundation queue. Safe to call from any thread.
pub(crate) fn load_video_tracks(
    asset: &AVURLAsset,
) -> mpsc::Receiver<Result<Option<SendBox<Retained<AVAssetTrack>>>, MediaError>> {
    let (tx, rx) = mpsc::channel();
    let block = RcBlock::new(
        move |tracks: *mut NSArray<AVAssetTrack>, error: *mut NSError| {
            // SAFETY: AVFoundation passes valid (or nil) objects to the completion handler.
            let result = match unsafe { Retained::retain(error) } {
                Some(error) => Err(media_error(&error)),
                None => Ok(unsafe { Retained::retain(tracks) }
                    .and_then(|tracks| tracks.firstObject())
                    .map(SendBox)),
            };
            let _ = tx.send(result);
        },
    );
    let media_type = unsafe { AVMediaTypeVideo }.expect("AVMediaTypeVideo is always defined");
    unsafe { asset.loadTracksWithMediaType_completionHandler(media_type, &block) };
    rx
}

/// Waits for a completion handler on a thread that is not the main thread.
pub(crate) fn wait<T>(rx: &mpsc::Receiver<T>, timeout: Duration) -> Option<T> {
    rx.recv_timeout(timeout).ok()
}

/// Inserts one camera's clips into `track` on the event axis, with empty edits for every gap and a
/// trailing empty edit up to `duration`. A clip with a hold shows its last frame for the hold.
pub(crate) fn insert_clips(
    track: &AVMutableCompositionTrack,
    timeline: &CameraTimeline,
    sources: &[&AVAssetTrack],
    duration: f64,
) -> Result<(), MediaError> {
    let mut cursor = 0.0;
    for (clip, source) in timeline.clips.iter().zip(sources) {
        if clip.start > cursor + 1e-6 {
            unsafe { track.insertEmptyTimeRange(range(cursor, clip.start - cursor)) };
        }
        insert_clip(track, clip, source)?;
        cursor = clip.end();
    }
    if duration > cursor + 1e-6 {
        unsafe { track.insertEmptyTimeRange(range(cursor, duration - cursor)) };
    }
    Ok(())
}

fn insert_clip(
    track: &AVMutableCompositionTrack,
    clip: &PlacedClip,
    source: &AVAssetTrack,
) -> Result<(), MediaError> {
    let insert = |source_start: f64, length: f64, at: f64| unsafe {
        track
            .insertTimeRange_ofTrack_atTime_error(range(source_start, length), source, cm(at))
            .map_err(|error| media_error(&error))
    };
    let frame = clip.info.frame_duration;
    if clip.hold > 0.0 && clip.duration > 2.0 * frame {
        // Everything but the last frame, then the last frame stretched over frame + hold.
        let body = clip.duration - frame;
        insert(clip.info.source_start, body, clip.start)?;
        insert(clip.info.source_start + body, frame, clip.start + body)?;
        unsafe {
            track.scaleTimeRange_toDuration(range(clip.start + body, frame), cm(frame + clip.hold))
        };
    } else {
        insert(clip.info.source_start, clip.duration, clip.start)?;
    }
    Ok(())
}

/// Builds a one-camera composition for playback.
pub(crate) fn camera_composition(
    timeline: &CameraTimeline,
    sources: &[&AVAssetTrack],
    duration: f64,
) -> Result<Retained<AVMutableComposition>, MediaError> {
    let composition = unsafe { AVMutableComposition::composition() };
    let track = add_video_track(&composition)?;
    insert_clips(&track, timeline, sources, duration)?;
    if let Some(first) = sources.first() {
        unsafe { track.setPreferredTransform(first.preferredTransform()) };
    }
    Ok(composition)
}

pub(crate) fn add_video_track(
    composition: &AVMutableComposition,
) -> Result<Retained<AVMutableCompositionTrack>, MediaError> {
    let media_type = unsafe { AVMediaTypeVideo }.expect("AVMediaTypeVideo is always defined");
    // kCMPersistentTrackID_Invalid (0) lets the composition pick an id.
    unsafe { composition.addMutableTrackWithMediaType_preferredTrackID(media_type, 0) }
        .ok_or_else(|| local_error("could not add a composition track"))
}

/// objc2-av-foundation returns `Retained<CVPixelBuffer>` (+1, copy family); the app and GPUI use
/// `CFRetained<CVPixelBuffer>`. Transfers the +1 without an extra retain/release.
pub(crate) fn into_cf(buffer: Retained<CVPixelBuffer>) -> CFRetained<CVPixelBuffer> {
    let raw = Retained::into_raw(buffer);
    // SAFETY: `raw` is a valid +1 CVPixelBuffer that we now own through CFRetained.
    unsafe { CFRetained::from_raw(NonNull::new_unchecked(raw)) }
}

pub(crate) fn affine_parts(t: CGAffineTransform) -> [f64; 6] {
    [t.a, t.b, t.c, t.d, t.tx, t.ty]
}

/// Collects AVFoundation completion results on any queue for the main thread to drain.
pub(crate) type Inbox<T> = std::sync::Arc<std::sync::Mutex<Vec<T>>>;
