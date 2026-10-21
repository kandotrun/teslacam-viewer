//! AVFoundation-backed playback and export for the TeslaCam Mac app.

mod av;
mod engine;
mod frame;
pub mod input;
pub mod layout;
mod playback;
mod probe;
mod representative;
pub mod run_loop;
pub mod status;
pub mod sync;
pub mod timeline;

pub use frame::{KEEPALIVE_FRAMES, VideoFrame, host_now};
pub use input::{CameraId, CameraMap, EventInput, SegmentInput};
pub use playback::{EventPlayback, PlaybackEvent, RATE_RANGE};
pub use probe::ProbeJob;
pub use representative::RepresentativePlayer;
pub use status::{CameraStatus, FileFault, MediaError};
