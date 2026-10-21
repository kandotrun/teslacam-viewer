//! Composite export layout, ported from the web app's `composite-layout.ts`.
//! Pure logic (belongs in the domain core crate in the real app).

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub const fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect { x, y, width, height }
}

pub const COMPOSITE_WIDTH: f64 = 1280.0;
pub const COMPOSITE_HEIGHT: f64 = 720.0;
pub const VEHICLE: Rect = rect(496.0, 260.0, 288.0, 184.0);

/// (camera index in TeslaCam order, slot). Order matches the web `cameraFrames`.
pub const CAMERA_SLOTS: [(usize, Rect); 6] = [
    (0, rect(400.0, 12.0, 480.0, 216.0)), // front
    (2, rect(12.0, 180.0, 384.0, 216.0)), // left_pillar
    (4, rect(884.0, 180.0, 384.0, 216.0)), // right_pillar
    (3, rect(12.0, 492.0, 384.0, 216.0)), // left_repeater
    (1, rect(400.0, 492.0, 480.0, 216.0)), // back
    (5, rect(884.0, 492.0, 384.0, 216.0)), // right_repeater
];

/// Fit `source` inside `destination`, centred, preserving aspect ratio.
pub fn contain_rect(source_width: f64, source_height: f64, destination: Rect) -> Rect {
    if source_width <= 0.0 || source_height <= 0.0 {
        return destination;
    }
    let scale = (destination.width / source_width).min(destination.height / source_height);
    let width = source_width * scale;
    let height = source_height * scale;
    Rect {
        x: destination.x + (destination.width - width) / 2.0,
        y: destination.y + (destination.height - height) / 2.0,
        width,
        height,
    }
}

/// Affine transform [a, b, c, d, tx, ty] mapping a source frame into `contain_rect`.
pub fn contain_transform(source_width: f64, source_height: f64, destination: Rect) -> [f64; 6] {
    let fitted = contain_rect(source_width, source_height, destination);
    let scale = fitted.width / source_width;
    [scale, 0.0, 0.0, scale, fitted.x, fitted.y]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contain_fits_4_3_camera_into_front_slot() {
        let r = contain_rect(1280.0, 960.0, rect(400.0, 12.0, 480.0, 216.0));
        assert_eq!(r, rect(496.0, 12.0, 288.0, 216.0));
    }

    #[test]
    fn contain_fits_vehicle_png_into_vehicle_slot() {
        let r = contain_rect(360.0, 720.0, VEHICLE);
        assert_eq!(r, rect(594.0, 260.0, 92.0, 184.0));
    }

    #[test]
    fn degenerate_source_returns_destination() {
        assert_eq!(contain_rect(0.0, 10.0, VEHICLE), VEHICLE);
    }

    #[test]
    fn transform_scales_and_translates() {
        let t = contain_transform(1280.0, 960.0, rect(12.0, 180.0, 384.0, 216.0));
        assert_eq!(t, [0.225, 0.0, 0.0, 0.225, 60.0, 180.0]);
    }
}
