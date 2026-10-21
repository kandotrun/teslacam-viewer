//! AVFoundation-backed playback and export for the TeslaCam Mac app.
//!
//! - [`EventPlayback`]: six `AVPlayer`s, one per camera, playing one event in sync on the wall-clock
//!   event timeline, with per-camera window states and decoded frames for rendering.
//! - [`RepresentativePlayer`]: a single player for an event's `event.mp4` with the same frame API.
//! - [`CompositeExporter`]: the 1280 × 720 / 30 fps H.264 composite of DESIGN.md "Export frame".
//! - [`ProbeJob`]: per-file durations, display sizes and decodability.
//!
//! # Threading
//!
//! AVFoundation playback objects are main-thread only. Every type that owns players or an export
//! session takes an [`objc2::MainThreadMarker`] and is `!Send`. Probing files, composing clips and
//! drawing the export overlay run on worker threads; their results are picked up by the owner's
//! `tick` / `poll` method on the main thread, which also applies AVFoundation completion handlers.
//! Nothing in this crate blocks the main thread or pumps the run loop, except the explicit helpers in
//! [`run_loop`] that exist for tests and command-line tools (never call them from a GPUI callback:
//! they re-enter the app).
//!
//! # Frames
//!
//! Frames are [`VideoFrame`]s: retained, IOSurface-backed `420f` `CVPixelBuffer`s plus their display
//! size. This crate has no GPUI dependency; the app hands `VideoFrame::buffer` to `gpui::surface`.
//!
//! # Time
//!
//! `now` arguments are host seconds ([`host_now`], the `CACurrentMediaTime` domain of the host time
//! clock the players run on). Positions are event seconds on the wall-clock timeline of
//! [`timeline::plan_event_timeline`].
//!
//! # Inputs
//!
//! Camera ids and per-camera maps are `teslacam-core`'s ([`CameraId`], [`CameraMap`]). An event is
//! handed over as an [`EventInput`]: its segments with their wall-clock offsets and file paths, built
//! from a core event and its `EventTimeline` by [`EventInput::from_event`].
#![warn(missing_docs)]

mod av;
mod engine;
mod export;
mod frame;
pub mod input;
pub mod layout;
pub mod overlay;
mod playback;
mod probe;
mod representative;
pub mod run_loop;
pub mod status;
pub mod sync;
pub mod timeline;

pub use export::{CompositeExporter, ExportError, ExportJob, ExportState};
pub use frame::{KEEPALIVE_FRAMES, VideoFrame, host_now};
pub use input::{CameraId, CameraMap, EventInput, SegmentInput};
pub use playback::{EventPlayback, PlaybackEvent, RATE_RANGE};
pub use probe::ProbeJob;
pub use representative::RepresentativePlayer;
pub use status::{CameraStatus, FileFault, MediaError};
