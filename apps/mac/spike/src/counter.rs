//! Decoder for the frame counter burned into the spike fixtures.
//!
//! Layout (see `scripts/gen-fixtures.sh`): 16x16 blocks in the top-left corner.
//! Row 0 holds 16 counter bits (LSB first) followed by a parity block; row 1
//! holds `camera index + 1` in 3 bits. Pure logic: it works on a luma plane.

pub const BLOCK: usize = 16;
pub const COUNTER_BITS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    /// Event-wide frame index (segment * 2160 + frame within segment).
    pub frame: u32,
    /// Camera index in TeslaCam order (front, back, left_pillar, ...).
    pub camera: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    PlaneTooSmall,
    Parity,
    NoCamera,
}

/// Luma plane view: `stride` bytes per row, at least `height` rows.
pub struct Luma<'a> {
    pub data: &'a [u8],
    pub stride: usize,
    pub width: usize,
    pub height: usize,
}

impl Luma<'_> {
    fn block_is_white(&self, bx: usize, by: usize) -> bool {
        // Average a 4x4 window at the block centre to ride out compression noise.
        let cx = bx * BLOCK + BLOCK / 2 - 2;
        let cy = by * BLOCK + BLOCK / 2 - 2;
        let mut sum = 0u32;
        for y in cy..cy + 4 {
            let row = &self.data[y * self.stride..];
            for x in cx..cx + 4 {
                sum += row[x] as u32;
            }
        }
        sum / 16 >= 128
    }
}

pub fn decode(luma: &Luma<'_>) -> Result<Stamp, DecodeError> {
    let needed_w = (COUNTER_BITS + 1) * BLOCK;
    if luma.width < needed_w || luma.height < 2 * BLOCK || luma.stride < needed_w {
        return Err(DecodeError::PlaneTooSmall);
    }
    let mut frame = 0u32;
    let mut ones = 0u32;
    for bit in 0..COUNTER_BITS {
        if luma.block_is_white(bit, 0) {
            frame |= 1 << bit;
            ones += 1;
        }
    }
    let parity = luma.block_is_white(COUNTER_BITS, 0);
    if (ones % 2 == 1) != parity {
        return Err(DecodeError::Parity);
    }
    let mut id = 0u8;
    for bit in 0..3 {
        if luma.block_is_white(bit, 1) {
            id |= 1 << bit;
        }
    }
    if id == 0 {
        return Err(DecodeError::NoCamera);
    }
    Ok(Stamp {
        frame,
        camera: id - 1,
    })
}

#[cfg(test)]
pub(crate) fn synth_plane(frame: u32, camera: u8, width: usize, height: usize) -> Vec<u8> {
    let mut plane = vec![40u8; width * height];
    let mut paint = |bx: usize, by: usize, white: bool| {
        for y in by * BLOCK..(by + 1) * BLOCK {
            for x in bx * BLOCK..(bx + 1) * BLOCK {
                plane[y * width + x] = if white { 235 } else { 16 };
            }
        }
    };
    for bit in 0..COUNTER_BITS {
        paint(bit, 0, frame & (1 << bit) != 0);
    }
    paint(COUNTER_BITS, 0, frame.count_ones() % 2 == 1);
    let id = camera + 1;
    for bit in 0..3 {
        paint(bit, 1, id & (1 << bit) != 0);
    }
    plane
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_frame_and_camera() {
        let plane = synth_plane(2196, 2, 320, 64);
        let luma = Luma { data: &plane, stride: 320, width: 320, height: 64 };
        assert_eq!(decode(&luma), Ok(Stamp { frame: 2196, camera: 2 }));
    }

    #[test]
    fn decodes_all_counter_values_used_by_fixtures() {
        for frame in [0, 1, 2159, 2160, 4319, 65535] {
            let plane = synth_plane(frame, 5, 300, 40);
            let luma = Luma { data: &plane, stride: 300, width: 300, height: 40 };
            assert_eq!(decode(&luma).unwrap().frame, frame);
        }
    }

    #[test]
    fn rejects_parity_mismatch() {
        let mut plane = synth_plane(7, 0, 320, 64);
        // Flip bit 0 to black without updating parity.
        for y in 0..BLOCK {
            for x in 0..BLOCK {
                plane[y * 320 + x] = 16;
            }
        }
        let luma = Luma { data: &plane, stride: 320, width: 320, height: 64 };
        assert_eq!(decode(&luma), Err(DecodeError::Parity));
    }

    #[test]
    fn rejects_small_plane() {
        let plane = vec![0u8; 100 * 10];
        let luma = Luma { data: &plane, stride: 100, width: 100, height: 10 };
        assert_eq!(decode(&luma), Err(DecodeError::PlaneTooSmall));
    }
}
