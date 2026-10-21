//! Development (`TESLACAM_SYNC_PROBE=1`): reads the frame counter that `scripts/gen-fixtures.sh`
//! burns into the media fixtures from the frames the stage is about to draw, and reports how many
//! frames apart the cameras are. The decoder is the media tests' (`counter.rs`); the statistics are
//! pure.

use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetBaseAddressOfPlane, CVPixelBufferGetBytesPerRowOfPlane,
    CVPixelBufferGetHeightOfPlane, CVPixelBufferGetWidthOfPlane, CVPixelBufferLockBaseAddress,
    CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
};

const BLOCK: usize = 16;
const ORIGIN: (usize, usize) = (32, 32);
const COUNTER_BITS: usize = 16;

/// The event-wide frame index burned into a fixture frame (`None` when the counter is unreadable,
/// e.g. on footage that is not a fixture).
pub fn decode_frame(buffer: &CVPixelBuffer) -> Option<u32> {
    // SAFETY: the buffer is locked read-only for the duration of the read and the slice covers
    // `stride × height` bytes of plane 0.
    unsafe { CVPixelBufferLockBaseAddress(buffer, CVPixelBufferLockFlags::ReadOnly) };
    let base = CVPixelBufferGetBaseAddressOfPlane(buffer, 0) as *const u8;
    let stride = CVPixelBufferGetBytesPerRowOfPlane(buffer, 0);
    let width = CVPixelBufferGetWidthOfPlane(buffer, 0);
    let height = CVPixelBufferGetHeightOfPlane(buffer, 0).min(96);
    let frame = (!base.is_null()).then(|| {
        let luma = unsafe { std::slice::from_raw_parts(base, stride * height) };
        decode_luma(luma, stride, width, height)
    });
    unsafe { CVPixelBufferUnlockBaseAddress(buffer, CVPixelBufferLockFlags::ReadOnly) };
    frame.flatten()
}

fn decode_luma(luma: &[u8], stride: usize, width: usize, height: usize) -> Option<u32> {
    let white = |bx: usize, by: usize| -> Option<bool> {
        let cx = ORIGIN.0 + bx * BLOCK + BLOCK / 2;
        let cy = ORIGIN.1 + by * BLOCK + BLOCK / 2;
        if cx + 2 > width || cy + 2 > height {
            return None;
        }
        let sum: u32 = (cy - 2..cy + 2)
            .flat_map(|y| (cx - 2..cx + 2).map(move |x| u32::from(luma[y * stride + x])))
            .sum();
        Some(sum / 16 >= 128)
    };
    let mut frame = 0u32;
    let mut ones = 0;
    for bit in 0..COUNTER_BITS {
        if white(bit, 0)? {
            frame |= 1 << bit;
            ones += 1;
        }
    }
    let parity_ok = (ones % 2 == 1) == white(COUNTER_BITS, 0)?;
    let camera_id = (0..3).try_fold(0u8, |id, bit| Some(id | (u8::from(white(bit, 1)?) << bit)))?;
    (parity_ok && camera_id != 0).then_some(frame)
}

/// Spread statistics over the samples taken while playing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SyncStats {
    /// Samples taken.
    pub samples: u64,
    /// Samples where every camera with a frame had a readable counter (at least two cameras).
    pub complete: u64,
    /// Complete samples by spread in frames: 0, 1, 2, 3 or more.
    pub histogram: [u64; 4],
    /// The largest spread seen.
    pub max_spread: u32,
}

impl SyncStats {
    /// Records one sample: the counters read from the frames drawn together (`None` = unreadable).
    pub fn record(&mut self, frames: &[Option<u32>]) {
        self.samples += 1;
        if frames.len() < 2 || frames.iter().any(Option::is_none) {
            return;
        }
        let values = frames.iter().flatten();
        let (min, max) = values.fold((u32::MAX, 0), |(min, max), frame| {
            (min.min(*frame), max.max(*frame))
        });
        let spread = max - min;
        self.complete += 1;
        self.histogram[(spread as usize).min(3)] += 1;
        self.max_spread = self.max_spread.max(spread);
    }

    /// One line for the log.
    pub fn summary(&self) -> String {
        format!(
            "sync: samples={} complete={} spread0={} spread1={} spread2={} spread3+={} max={}",
            self.samples,
            self.complete,
            self.histogram[0],
            self.histogram[1],
            self.histogram[2],
            self.histogram[3],
            self.max_spread
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spreads_are_counted_per_complete_sample() {
        let mut stats = SyncStats::default();
        stats.record(&[Some(100), Some(100), Some(100)]);
        stats.record(&[Some(100), Some(101)]);
        stats.record(&[Some(90), Some(100)]);
        stats.record(&[Some(100), None]);
        stats.record(&[Some(100)]);
        assert_eq!(stats.samples, 5);
        assert_eq!(stats.complete, 3);
        assert_eq!(stats.histogram, [1, 1, 0, 1]);
        assert_eq!(stats.max_spread, 10);
        assert!(
            stats
                .summary()
                .starts_with("sync: samples=5 complete=3 spread0=1")
        );
    }

    #[test]
    fn the_counter_is_read_from_its_blocks() {
        // A 320 × 96 grey frame with frame 5 (bits 0 and 2, even parity) of camera 3 (bits 0, 1).
        let (width, height) = (320, 96);
        let mut luma = vec![0u8; width * height];
        let fill = |luma: &mut [u8], bx: usize, by: usize| {
            for y in 0..BLOCK {
                for x in 0..BLOCK {
                    luma[(ORIGIN.1 + by * BLOCK + y) * width + ORIGIN.0 + bx * BLOCK + x] = 255;
                }
            }
        };
        for (bx, by) in [(0, 0), (2, 0), (0, 1), (1, 1)] {
            fill(&mut luma, bx, by);
        }
        assert_eq!(decode_luma(&luma, width, width, height), Some(5));
        // Wrong parity: unreadable.
        fill(&mut luma, COUNTER_BITS, 0);
        assert_eq!(decode_luma(&luma, width, width, height), None);
    }
}
