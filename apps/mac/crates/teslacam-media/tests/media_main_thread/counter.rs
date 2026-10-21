//! Decoder for the frame counter burned into the fixtures (see `apps/mac/scripts/gen-fixtures.sh`).
//!
//! 16 × 16 blocks starting at [`ORIGIN`]: row 0 holds 16 counter bits (LSB first) and a parity
//! block, row 1 holds `camera index + 1` in 3 bits.

use objc2_core_video::{
    CVPixelBuffer, CVPixelBufferGetBaseAddressOfPlane, CVPixelBufferGetBytesPerRowOfPlane,
    CVPixelBufferGetHeightOfPlane, CVPixelBufferGetWidthOfPlane, CVPixelBufferLockBaseAddress,
    CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
};

pub const BLOCK: f64 = 16.0;
/// Top-left corner of the counter in a source frame.
pub const ORIGIN: (f64, f64) = (32.0, 32.0);
const COUNTER_BITS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    /// Event-wide frame index (`round(offset * 36) + n`).
    pub frame: u32,
    /// `camera index + 1` (1…6 for cameras, 7 for event.mp4).
    pub id: u8,
}

/// A grey image (one byte per pixel) with an origin and scale for the counter blocks.
pub struct Luma<'a> {
    pub data: &'a [u8],
    pub stride: usize,
    pub width: usize,
    pub height: usize,
}

/// Decodes a counter whose top-left corner is at `(x0, y0)`, drawn at `scale` × its 16 px blocks.
pub fn decode_at(luma: &Luma<'_>, x0: f64, y0: f64, scale: f64) -> Option<Stamp> {
    let white = |bx: usize, by: usize| -> Option<bool> {
        let cx = x0 + (bx as f64 * BLOCK + BLOCK / 2.0) * scale;
        let cy = y0 + (by as f64 * BLOCK + BLOCK / 2.0) * scale;
        // Average a small window at the block centre to ride out compression noise.
        let r = (scale * 2.0).max(1.0) as i64;
        let (mut sum, mut n) = (0u32, 0u32);
        for y in (cy as i64 - r)..(cy as i64 + r) {
            for x in (cx as i64 - r)..(cx as i64 + r) {
                if x < 0 || y < 0 || x as usize >= luma.width || y as usize >= luma.height {
                    return None;
                }
                sum += u32::from(luma.data[y as usize * luma.stride + x as usize]);
                n += 1;
            }
        }
        Some(sum / n.max(1) >= 128)
    };
    let mut frame = 0u32;
    let mut ones = 0;
    for bit in 0..COUNTER_BITS {
        if white(bit, 0)? {
            frame |= 1 << bit;
            ones += 1;
        }
    }
    if (ones % 2 == 1) != white(COUNTER_BITS, 0)? {
        return None;
    }
    let mut id = 0u8;
    for bit in 0..3 {
        if white(bit, 1)? {
            id |= 1 << bit;
        }
    }
    (id != 0).then_some(Stamp { frame, id })
}

/// Reads the counter from the Y plane of a 420f pixel buffer.
pub fn decode_buffer(buffer: &CVPixelBuffer) -> Option<Stamp> {
    unsafe { CVPixelBufferLockBaseAddress(buffer, CVPixelBufferLockFlags::ReadOnly) };
    let base = CVPixelBufferGetBaseAddressOfPlane(buffer, 0) as *const u8;
    let stride = CVPixelBufferGetBytesPerRowOfPlane(buffer, 0);
    let width = CVPixelBufferGetWidthOfPlane(buffer, 0);
    let height = CVPixelBufferGetHeightOfPlane(buffer, 0).min(96);
    let stamp = (!base.is_null()).then(|| {
        let data = unsafe { std::slice::from_raw_parts(base, stride * height) };
        decode_at(
            &Luma {
                data,
                stride,
                width,
                height,
            },
            ORIGIN.0,
            ORIGIN.1,
            1.0,
        )
    });
    unsafe { CVPixelBufferUnlockBaseAddress(buffer, CVPixelBufferLockFlags::ReadOnly) };
    stamp.flatten()
}
