//! Turning a drawn bitmap into image bytes GPUI can decode: the export dialog shows the export
//! frame's overlay (`teslacam_media::overlay::render_overlay`, premultiplied RGBA with transparent
//! holes where the videos go) composited over the video colour, as an uncompressed 24-bit BMP.

/// Composites premultiplied RGBA `pixels` (`width × height`, top row first) over the opaque
/// `background` and encodes the result as a bottom-up 24-bit BMP.
pub fn opaque_bmp(width: usize, height: usize, pixels: &[u8], background: [u8; 3]) -> Vec<u8> {
    const HEADER: usize = 14 + 40;
    let row = (width * 3).div_ceil(4) * 4;
    let image_size = row * height;
    let file_size = HEADER + image_size;
    let mut bytes = Vec::with_capacity(file_size);
    bytes.extend_from_slice(b"BM");
    bytes.extend_from_slice(&(file_size as u32).to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&(HEADER as u32).to_le_bytes());
    bytes.extend_from_slice(&40u32.to_le_bytes());
    bytes.extend_from_slice(&(width as i32).to_le_bytes());
    bytes.extend_from_slice(&(height as i32).to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&24u16.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&(image_size as u32).to_le_bytes());
    bytes.extend_from_slice(&2_835i32.to_le_bytes());
    bytes.extend_from_slice(&2_835i32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    let over = |source: u8, alpha: u8, back: u8| {
        let value = u32::from(source) + u32::from(back) * (255 - u32::from(alpha)) / 255;
        value.min(255) as u8
    };
    for y in (0..height).rev() {
        let start = bytes.len();
        for x in 0..width {
            let index = (y * width + x) * 4;
            let [r, g, b, a] = [
                pixels[index],
                pixels[index + 1],
                pixels[index + 2],
                pixels[index + 3],
            ];
            bytes.extend_from_slice(&[
                over(b, a, background[2]),
                over(g, a, background[1]),
                over(r, a, background[0]),
            ]);
        }
        bytes.resize(start + row, 0);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_are_composited_and_written_bottom_up_in_bgr_rows_padded_to_four_bytes() {
        // Row 0: opaque red, transparent. Row 1: half-transparent white (premultiplied), opaque blue.
        let pixels = [
            255, 0, 0, 255, 0, 0, 0, 0, //
            128, 128, 128, 128, 0, 0, 255, 255,
        ];
        let bmp = opaque_bmp(2, 2, &pixels, [10, 20, 30]);
        assert_eq!(&bmp[..2], b"BM");
        assert_eq!(u32::from_le_bytes(bmp[2..6].try_into().unwrap()), 54 + 16);
        assert_eq!(u32::from_le_bytes(bmp[10..14].try_into().unwrap()), 54);
        assert_eq!(i32::from_le_bytes(bmp[18..22].try_into().unwrap()), 2);
        assert_eq!(i32::from_le_bytes(bmp[22..26].try_into().unwrap()), 2);
        assert_eq!(u16::from_le_bytes(bmp[28..30].try_into().unwrap()), 24);
        // Bottom row first: 128 + 30 × 127 / 255 = 142 (blue), 128 + 20 × 127 / 255 = 137, …
        assert_eq!(&bmp[54..62], &[142, 137, 132, 255, 0, 0, 0, 0]);
        assert_eq!(&bmp[62..70], &[0, 0, 255, 30, 20, 10, 0, 0]);
        assert_eq!(bmp.len(), 70);
    }
}
