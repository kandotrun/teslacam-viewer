//! Decoded video frames handed to the app for rendering.

use objc2_core_foundation::CFRetained;
use objc2_core_video::CVPixelBuffer;

/// One decoded frame ready to draw.
///
/// `buffer` is a retained, IOSurface-backed `420f` (`kCVPixelFormatType_420YpCbCr8BiPlanarFullRange`)
/// pixel buffer, which is what GPUI's `surface()` element draws. The player keeps the last few
/// buffers it handed out alive for a few more frames (see [`KEEPALIVE_FRAMES`]) because GPUI's Metal
/// renderer releases its texture before the GPU has finished sampling it.
#[derive(Clone)]
pub struct VideoFrame {
    /// The decoded pixels.
    pub buffer: CFRetained<CVPixelBuffer>,
    /// Width of the picture as it should be displayed (after the clip's preferred transform).
    pub display_width: f64,
    /// Height of the picture as it should be displayed.
    pub display_height: f64,
}

impl std::fmt::Debug for VideoFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VideoFrame")
            .field("display_width", &self.display_width)
            .field("display_height", &self.display_height)
            .finish_non_exhaustive()
    }
}

/// How many previously shown buffers each player keeps alive after replacing them.
pub const KEEPALIVE_FRAMES: usize = 3;

/// The current host time in seconds (`CACurrentMediaTime`), the time base of every `now` argument
/// in this crate. Pass the time the frame will be presented at when it is known.
pub fn host_now() -> f64 {
    objc2_quartz_core::CACurrentMediaTime()
}
