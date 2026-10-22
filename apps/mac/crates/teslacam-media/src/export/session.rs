//! [`ExportBackend::ExportSession`](super::ExportBackend::ExportSession): the v0.1.0 pipeline.
//!
//! One `AVMutableComposition` with a video track per camera (clips on the wall-clock event timeline)
//! goes through an `AVMutableVideoComposition` that contain-fits every camera into its slot, and an
//! `AVVideoCompositionCoreAnimationTool` that lays the static overlay image on top. Missing tiles sit
//! between the video and the overlay and are switched on only for the ranges a camera has no video,
//! with discrete opacity keyframes, so no layer is redrawn per frame. `AVAssetExportSession` encodes
//! with its 1280 × 720 preset. The composition is built on the worker thread, the session is started
//! and polled on the main thread.

use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use block2::RcBlock;
use objc2::{
    AnyThread,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject},
};
use objc2_av_foundation::{
    AVAssetExportPreset1280x720, AVAssetExportSession, AVAssetExportSessionStatus, AVAssetTrack,
    AVCoreAnimationBeginTimeAtZero, AVFileTypeMPEG4, AVMutableComposition,
    AVMutableCompositionTrack, AVMutableVideoComposition, AVMutableVideoCompositionInstruction,
    AVMutableVideoCompositionLayerInstruction, AVVideoCompositionCoreAnimationTool,
    AVVideoCompositionInstructionProtocol, AVVideoCompositionLayerInstruction,
};
use objc2_core_foundation::{CGAffineTransform, CGPoint, CGRect, CGSize};
use objc2_core_graphics::{CGColor, CGImage};
use objc2_core_media::CMTime;
use objc2_foundation::{NSArray, NSNumber, NSString};
use objc2_quartz_core::{
    CAKeyframeAnimation, CALayer, CAMediaTiming, kCAAnimationDiscrete, kCAFillModeBoth,
};

use super::{
    Encoding, ExportError, MissingTile, Prepared, failure, media_failure, plan::Keyframes,
};
use crate::{
    av,
    input::{CameraId, CameraMap},
    layout::{ExportLayout, Rect, fit_transform},
};

/// One camera's composition track and the source track (with its start on the event axis) of each
/// of its clips.
struct CameraTrack {
    track: Retained<AVMutableCompositionTrack>,
    sources: Vec<(f64, Retained<AVAssetTrack>)>,
}

/// The six-track composition, built on the worker thread.
pub(super) struct Composed {
    prepared: Prepared,
    composition: Retained<AVMutableComposition>,
    /// Composition track per camera with video.
    tracks: CameraMap<Option<CameraTrack>>,
}

/// Worker thread: the composition with one track per camera that has video.
pub(super) fn compose(prepared: Prepared) -> Result<Composed, ExportError> {
    let composition = unsafe { AVMutableComposition::composition() };
    let mut tracks = CameraMap::from_fn(|_| None);
    for camera in CameraId::ALL {
        let camera_timeline = &prepared.timeline.cameras[camera];
        if !camera_timeline.has_video() {
            continue;
        }
        let sources = prepared.sources(camera);
        let track = av::add_video_track(&composition).map_err(media_failure)?;
        let refs: Vec<&AVAssetTrack> = sources.iter().map(|(_, track)| &**track).collect();
        av::insert_clips(&track, camera_timeline, &refs, prepared.timeline.duration)
            .map_err(media_failure)?;
        tracks[camera] = Some(CameraTrack { track, sources });
    }
    Ok(Composed {
        prepared,
        composition,
        tracks,
    })
}

/// A running `AVAssetExportSession` (main thread).
struct Running {
    session: Retained<AVAssetExportSession>,
    done: Arc<AtomicBool>,
    _composed: Composed,
}

impl Encoding for Running {
    fn progress(&self) -> f32 {
        unsafe { self.session.progress() }
    }

    fn outcome(&mut self) -> Option<Result<(), ExportError>> {
        if !self.done.load(Ordering::SeqCst) {
            return None;
        }
        Some(match unsafe { self.session.status() } {
            AVAssetExportSessionStatus::Completed => Ok(()),
            AVAssetExportSessionStatus::Cancelled => Err(ExportError::Aborted),
            _ => {
                let error = unsafe { self.session.error() }.map(|e| av::media_error(&e));
                Err(ExportError::RecordingFailed(error))
            }
        })
    }

    fn cancel(&mut self) {
        unsafe { self.session.cancelExport() };
    }

    fn abandon(&mut self, output: &Path) {
        unsafe { self.session.cancelExport() };
        let _ = std::fs::remove_file(output);
    }
}

fn image_layer(image: &CGImage, frame: CGRect) -> Retained<CALayer> {
    let layer = CALayer::new();
    layer.setFrame(frame);
    // SAFETY: CALayer.contents accepts a CGImage.
    unsafe { layer.setContents(Some(&*(image as *const CGImage as *const AnyObject))) };
    layer
}

fn cg_rect(rect: Rect) -> CGRect {
    CGRect {
        origin: CGPoint {
            x: rect.x,
            y: rect.y,
        },
        size: CGSize {
            width: rect.width,
            height: rect.height,
        },
    }
}

fn opacity_animation(plan: &Keyframes, duration: f64) -> Retained<CAKeyframeAnimation> {
    let animation = CAKeyframeAnimation::animationWithKeyPath(Some(&NSString::from_str("opacity")));
    let values: Vec<Retained<NSNumber>> =
        plan.values.iter().map(|v| NSNumber::new_f64(*v)).collect();
    let key_times: Vec<Retained<NSNumber>> = plan
        .key_times
        .iter()
        .map(|t| NSNumber::new_f64(*t))
        .collect();
    unsafe {
        let values = NSArray::from_retained_slice(&values);
        // SAFETY: an NSArray of NSNumbers is an NSArray of objects; the element type is only static.
        let values = &*(Retained::as_ptr(&values) as *const NSArray);
        animation.setValues(Some(values));
        animation.setKeyTimes(Some(&NSArray::from_retained_slice(&key_times)));
        animation.setCalculationMode(kCAAnimationDiscrete);
        // AVFoundation treats a beginTime of 0 as "now"; this constant means the start of the video.
        animation.setBeginTime(AVCoreAnimationBeginTimeAtZero);
        animation.setFillMode(kCAFillModeBoth);
    }
    animation.setDuration(duration);
    animation.setRemovedOnCompletion(false);
    animation
}

/// Main thread: video composition, CoreAnimation layers and the export session.
pub(super) fn start(
    composed: Composed,
    layout: &ExportLayout,
    output: &Path,
) -> Result<Box<dyn Encoding>, ExportError> {
    let prepared = &composed.prepared;
    let plan = &prepared.plan;
    let (width, height) = (layout.width, layout.height);

    let video_composition = unsafe { AVMutableVideoComposition::videoComposition() };
    unsafe {
        video_composition.setRenderSize(CGSize { width, height });
        video_composition.setFrameDuration(CMTime::new(1, layout.fps as i32));
    }
    let instruction =
        unsafe { AVMutableVideoCompositionInstruction::videoCompositionInstruction() };
    let floor = CGColor::new_srgb(layout.floor.r, layout.floor.g, layout.floor.b, 1.0);
    unsafe {
        instruction.setTimeRange(av::range(0.0, plan.duration));
        instruction.setBackgroundColor(Some(&floor));
    }
    let mut layer_instructions: Vec<Retained<AVVideoCompositionLayerInstruction>> = Vec::new();
    for camera in CameraId::ALL {
        let Some(CameraTrack { track, sources }) = &composed.tracks[camera] else {
            continue;
        };
        let instruction = unsafe {
            AVMutableVideoCompositionLayerInstruction::videoCompositionLayerInstructionWithAssetTrack(track)
        };
        for (index, (start, source)) in sources.iter().enumerate() {
            let (natural, preferred) =
                unsafe { (source.naturalSize(), source.preferredTransform()) };
            let [a, b, c, d, tx, ty] = fit_transform(
                (natural.width, natural.height),
                av::affine_parts(preferred),
                layout.slots[camera],
            );
            let time = if index == 0 { 0.0 } else { *start };
            unsafe {
                instruction
                    .setTransform_atTime(CGAffineTransform { a, b, c, d, tx, ty }, av::cm(time))
            };
        }
        layer_instructions.push(Retained::into_super(instruction));
    }
    unsafe {
        instruction.setLayerInstructions(&NSArray::from_retained_slice(&layer_instructions));
        let instruction: Retained<ProtocolObject<dyn AVVideoCompositionInstructionProtocol>> =
            ProtocolObject::from_retained(instruction);
        video_composition.setInstructions(&NSArray::from_retained_slice(&[instruction]));
    }

    // Layers, bottom to top: video, missing tiles, overlay. CALayer geometry on macOS has its origin
    // at the bottom-left.
    let full = cg_rect(Rect::new(0.0, 0.0, width, height));
    let parent = CALayer::new();
    parent.setFrame(full);
    let video = CALayer::new();
    video.setFrame(full);
    parent.addSublayer(&video);
    for camera in CameraId::ALL {
        let Some(tile) = &prepared.missing[camera] else {
            continue;
        };
        let missing = &plan.cameras[camera].missing;
        let layer = image_layer(
            &tile.image,
            cg_rect(plan.cameras[camera].hole.bottom_left(height)),
        );
        match missing {
            MissingTile::Never => continue,
            MissingTile::Always => layer.setOpacity(1.0),
            MissingTile::During(_) => {
                let keyframes = missing
                    .keyframes(plan.duration)
                    .expect("ranges have keyframes");
                layer.setOpacity(0.0);
                layer.addAnimation_forKey(
                    &opacity_animation(&keyframes, plan.duration),
                    Some(&NSString::from_str("missing")),
                );
            }
        }
        parent.addSublayer(&layer);
    }
    parent.addSublayer(&image_layer(&prepared.overlay.image, full));
    let tool = unsafe {
        AVVideoCompositionCoreAnimationTool::videoCompositionCoreAnimationToolWithPostProcessingAsVideoLayer_inLayer(&video, &parent)
    };
    unsafe { video_composition.setAnimationTool(Some(&tool)) };

    let _ = std::fs::remove_file(output);
    let preset = unsafe { AVAssetExportPreset1280x720 };
    let session = unsafe {
        AVAssetExportSession::initWithAsset_presetName(
            AVAssetExportSession::alloc(),
            &composed.composition,
            preset,
        )
    }
    .ok_or_else(|| failure("could not create the export session"))?;
    unsafe {
        session.setOutputURL(Some(&av::file_url(output)));
        session.setOutputFileType(AVFileTypeMPEG4);
        session.setShouldOptimizeForNetworkUse(true);
        session.setVideoComposition(Some(&video_composition));
    }
    let done = Arc::new(AtomicBool::new(false));
    let flag = done.clone();
    let handler = RcBlock::new(move || flag.store(true, Ordering::SeqCst));
    unsafe { session.exportAsynchronouslyWithCompletionHandler(&handler) };
    Ok(Box::new(Running {
        session,
        done,
        _composed: composed,
    }))
}
