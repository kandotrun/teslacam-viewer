//! The static images of the export frame, drawn once per export with CoreGraphics and CoreText.
//!
//! - The overlay: `floor` (with the `floor-light` highlight behind the car) everywhere except rounded
//!   holes over the fitted video rects, plus the neutral field-of-view wedges, the car with its soft
//!   floor shadow and the camera label pills.
//! - The missing tile: `video-missing-a` with a centred `記録なし`.
//!
//! Rendering is thread-safe (own bitmap contexts only) and runs on the export worker thread. The same
//! overlay can serve as the layout thumbnail of the export dialog.

use std::ffi::c_void;

use objc2::{AnyThread, rc::Retained, runtime::AnyObject};
use objc2_app_kit::{
    NSFont, NSFontWeightBold, NSFontWeightMedium, NSFontWeightRegular, NSFontWeightSemibold,
};
use objc2_core_foundation::{
    CFAttributedString, CFData, CFIndex, CFRetained, CGAffineTransform, CGFloat, CGPoint, CGRect,
    CGSize,
};
use objc2_core_graphics::{
    CGBitmapContextCreate, CGBitmapContextCreateImage, CGBlendMode, CGColorRenderingIntent,
    CGColorSpace, CGContext, CGDataProvider, CGGradient, CGGradientDrawingOptions, CGImage,
    CGImageAlphaInfo, CGMutablePath, CGPath, kCGColorSpaceSRGB,
};
use objc2_core_text::{CTLine, kCTFontAttributeName, kCTForegroundColorFromContextAttributeName};
use objc2_foundation::{NSAttributedString, NSDictionary, NSNumber, NSString};

use crate::{
    av,
    input::{CameraId, CameraMap},
    layout::{ExportLayout, Rect, Rgba, contain_rect},
};

/// The car render shown in the middle of the export frame (`apps/web/public/vehicle-top.png`).
pub const VEHICLE_PNG: &[u8] = include_bytes!("../../../../web/public/vehicle-top.png");

/// An 8-bit premultiplied RGBA bitmap, top row first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// `width * height * 4` bytes, premultiplied RGBA.
    pub rgba: Vec<u8>,
}

impl Bitmap {
    /// `[r, g, b, a]` at `(x, y)` (premultiplied).
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let i = (y * self.width + x) * 4;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }
}

/// Why an overlay could not be drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayError {
    /// The car image is not a decodable PNG.
    VehicleImage,
    /// CoreGraphics could not create a bitmap context or image.
    Graphics,
}

/// A drawn image: the pixels (for checks and previews) and the `CGImage` for CoreAnimation.
pub(crate) struct Drawn {
    pub bitmap: Bitmap,
    pub image: CFRetained<CGImage>,
}

// SAFETY: a CGImage is immutable and documented as thread-safe; the bitmap is plain data.
unsafe impl Send for Drawn {}

/// Draws the overlay for the given fitted video rects (`holes`).
pub fn render_overlay(
    layout: &ExportLayout,
    holes: &CameraMap<Rect>,
    vehicle_png: &[u8],
) -> Result<Bitmap, OverlayError> {
    draw_overlay(layout, holes, vehicle_png).map(|drawn| drawn.bitmap)
}

pub(crate) fn draw_overlay(
    layout: &ExportLayout,
    holes: &CameraMap<Rect>,
    vehicle_png: &[u8],
) -> Result<Drawn, OverlayError> {
    let vehicle = decode_png(vehicle_png).ok_or(OverlayError::VehicleImage)?;
    let font = label_font(layout);
    draw(layout.width as usize, layout.height as usize, |canvas| {
        canvas.floor(layout);
        for camera in CameraId::ALL {
            let wedge = layout.wedge(camera, holes[camera]);
            canvas.wedge(&wedge.points, wedge.gradient, layout.wedge);
        }
        canvas.car(layout, &vehicle);
        for camera in CameraId::ALL {
            canvas.clear_rounded(holes[camera], layout.video_corner_radius);
        }
        for camera in CameraId::ALL {
            let line = text_line(&layout.labels[camera], &font);
            let label = layout.label_geometry(holes[camera], line.width);
            canvas.fill_rounded(label.pill, label.pill.height / 2.0, layout.label.scrim);
            canvas.text(&line, label.text_x, label.pill, layout.label.text);
        }
    })
}

/// Draws the missing tile for a window of `size`.
pub(crate) fn draw_missing_tile(
    layout: &ExportLayout,
    size: (f64, f64),
) -> Result<Drawn, OverlayError> {
    let font = label_font(layout);
    let (width, height) = (
        size.0.round().max(1.0) as usize,
        size.1.round().max(1.0) as usize,
    );
    draw(width, height, |canvas| {
        let bounds = Rect::new(0.0, 0.0, width as f64, height as f64);
        canvas.fill_rect(bounds, layout.missing.fill);
        let line = text_line(&layout.missing.message, &font);
        let x = (bounds.width - line.width) / 2.0;
        let band = Rect::new(0.0, 0.0, bounds.width, bounds.height);
        canvas.text(&line, x, band, layout.missing.text);
    })
}

fn decode_png(png: &[u8]) -> Option<CFRetained<CGImage>> {
    let data = unsafe { CFData::new(None, png.as_ptr(), png.len() as CFIndex) }?;
    let provider = CGDataProvider::with_cf_data(Some(&data))?;
    unsafe {
        CGImage::with_png_data_provider(
            Some(&provider),
            std::ptr::null(),
            true,
            CGColorRenderingIntent::RenderingIntentDefault,
        )
    }
}

fn label_font(layout: &ExportLayout) -> Retained<NSFont> {
    let weight = unsafe {
        match layout.label.font_weight {
            0..=449 => NSFontWeightRegular,
            450..=549 => NSFontWeightMedium,
            550..=649 => NSFontWeightSemibold,
            _ => NSFontWeightBold,
        }
    };
    // The system font; CoreText falls back to Hiragino Sans for the Japanese glyphs.
    NSFont::systemFontOfSize_weight(layout.label.font_size, weight)
}

struct Line {
    line: CFRetained<CTLine>,
    width: f64,
    ascent: f64,
    descent: f64,
}

fn text_line(text: &str, font: &NSFont) -> Line {
    let from_context = NSNumber::new_bool(true);
    let attributes: Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::from_slices(
        &[
            av::ns_key(unsafe { kCTFontAttributeName }),
            av::ns_key(unsafe { kCTForegroundColorFromContextAttributeName }),
        ],
        &[font as &AnyObject, from_context.as_ref() as &AnyObject],
    );
    let string = unsafe {
        NSAttributedString::initWithString_attributes(
            NSAttributedString::alloc(),
            &NSString::from_str(text),
            Some(&attributes),
        )
    };
    // SAFETY: NSAttributedString and CFAttributedString are toll-free bridged.
    let cf = unsafe { &*(Retained::as_ptr(&string) as *const CFAttributedString) };
    let line = unsafe { CTLine::with_attributed_string(cf) };
    let (mut ascent, mut descent, mut leading): (CGFloat, CGFloat, CGFloat) = (0.0, 0.0, 0.0);
    let width = unsafe { line.typographic_bounds(&mut ascent, &mut descent, &mut leading) };
    Line {
        line,
        width,
        ascent,
        descent,
    }
}

fn cg_rect(r: Rect) -> CGRect {
    CGRect {
        origin: CGPoint { x: r.x, y: r.y },
        size: CGSize {
            width: r.width,
            height: r.height,
        },
    }
}

/// An axis-aligned ellipse.
#[derive(Clone, Copy)]
struct Ellipse {
    center: (f64, f64),
    rx: f64,
    ry: f64,
}

/// A CoreGraphics context with a top-left origin.
struct Canvas<'a> {
    ctx: &'a CGContext,
    space: &'a CGColorSpace,
}

impl Canvas<'_> {
    fn c(&self) -> Option<&CGContext> {
        Some(self.ctx)
    }

    fn set_fill(&self, color: Rgba) {
        CGContext::set_rgb_fill_color(self.c(), color.r, color.g, color.b, color.a);
    }

    fn fill_rect(&self, rect: Rect, color: Rgba) {
        self.set_fill(color);
        CGContext::fill_rect(self.c(), cg_rect(rect));
    }

    fn rounded(&self, rect: Rect, radius: f64) -> CFRetained<CGPath> {
        let radius = radius.min(rect.width / 2.0).min(rect.height / 2.0);
        unsafe { CGPath::with_rounded_rect(cg_rect(rect), radius, radius, std::ptr::null()) }
    }

    fn fill_rounded(&self, rect: Rect, radius: f64, color: Rgba) {
        self.set_fill(color);
        CGContext::add_path(self.c(), Some(&self.rounded(rect, radius)));
        CGContext::fill_path(self.c());
    }

    fn clear_rounded(&self, rect: Rect, radius: f64) {
        CGContext::set_blend_mode(self.c(), CGBlendMode::Clear);
        self.fill_rounded(
            rect,
            radius,
            Rgba {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        );
        CGContext::set_blend_mode(self.c(), CGBlendMode::Normal);
    }

    fn gradient(&self, from: Rgba, to: Rgba, end_location: f64) -> Option<CFRetained<CGGradient>> {
        let components = [from.r, from.g, from.b, from.a, to.r, to.g, to.b, to.a];
        let locations = [0.0, end_location];
        unsafe {
            CGGradient::with_color_components(
                Some(self.space),
                components.as_ptr(),
                locations.as_ptr(),
                2,
            )
        }
    }

    /// An elliptical radial gradient over `ellipse`, from `from` at its centre to `to` at `end`
    /// (a fraction of the radii); with `extend`, `to` also fills everything outside.
    fn radial(&self, ellipse: Ellipse, (from, to, end): (Rgba, Rgba, f64), extend: bool) {
        let Some(gradient) = self.gradient(from, to, end) else {
            return;
        };
        CGContext::save_g_state(self.c());
        CGContext::translate_ctm(self.c(), ellipse.center.0, ellipse.center.1);
        CGContext::scale_ctm(self.c(), ellipse.rx / ellipse.ry, 1.0);
        let options = if extend {
            CGGradientDrawingOptions::DrawsAfterEndLocation
        } else {
            CGGradientDrawingOptions::empty()
        };
        let origin = CGPoint { x: 0.0, y: 0.0 };
        CGContext::draw_radial_gradient(
            self.c(),
            Some(&gradient),
            origin,
            0.0,
            origin,
            ellipse.ry,
            options,
        );
        CGContext::restore_g_state(self.c());
    }

    fn floor(&self, layout: &ExportLayout) {
        let full = Rect::new(0.0, 0.0, layout.width, layout.height);
        self.fill_rect(full, layout.floor);
        // DESIGN.md: one radial highlight centred on the car (prototype: ellipse 70 % × 75 %,
        // floor-light at the centre fading to floor at 62 %).
        let center = layout.vehicle.center();
        let ellipse = Ellipse {
            center,
            rx: 0.70 * layout.width,
            ry: 0.75 * layout.height,
        };
        self.radial(ellipse, (layout.floor_light, layout.floor, 0.62), true);
    }

    fn wedge(&self, points: &[(f64, f64); 3], axis: ((f64, f64), (f64, f64)), color: Rgba) {
        let Some(gradient) = self.gradient(color, Rgba { a: 0.0, ..color }, 1.0) else {
            return;
        };
        let path = CGMutablePath::new();
        unsafe {
            CGMutablePath::move_to_point(Some(&path), std::ptr::null(), points[0].0, points[0].1);
            CGMutablePath::add_line_to_point(
                Some(&path),
                std::ptr::null(),
                points[1].0,
                points[1].1,
            );
            CGMutablePath::add_line_to_point(
                Some(&path),
                std::ptr::null(),
                points[2].0,
                points[2].1,
            );
        }
        CGMutablePath::close_subpath(Some(&path));
        CGContext::save_g_state(self.c());
        CGContext::add_path(self.c(), Some(&path));
        CGContext::clip(self.c());
        let (start, end) = (
            CGPoint {
                x: axis.0.0,
                y: axis.0.1,
            },
            CGPoint {
                x: axis.1.0,
                y: axis.1.1,
            },
        );
        CGContext::draw_linear_gradient(
            self.c(),
            Some(&gradient),
            start,
            end,
            CGGradientDrawingOptions::empty(),
        );
        CGContext::restore_g_state(self.c());
    }

    fn car(&self, layout: &ExportLayout, vehicle: &CGImage) {
        let v = layout.vehicle;
        // The prototype's `.car::before`: an ellipse inset 6 % / −18 % / 2 % around the car box with
        // `ink` at 22 % fading out (its 6 px blur is approximated by the gradient itself).
        let shadow = Rect::new(
            v.x - 0.18 * v.width,
            v.y + 0.06 * v.height,
            1.36 * v.width,
            0.92 * v.height,
        );
        let ink = Rgba {
            r: 0x17 as f64 / 255.0,
            g: 0x1A as f64 / 255.0,
            b: 0x20 as f64 / 255.0,
            a: 0.22,
        };
        let ellipse = Ellipse {
            center: shadow.center(),
            rx: shadow.width / 2.0,
            ry: shadow.height / 2.0,
        };
        self.radial(ellipse, (ink, Rgba { a: 0.0, ..ink }, 1.0), false);
        let (w, h) = (
            CGImage::width(Some(vehicle)) as f64,
            CGImage::height(Some(vehicle)) as f64,
        );
        let fitted = contain_rect(w, h, v);
        // The context is flipped; flip back locally so the image is drawn upright.
        CGContext::save_g_state(self.c());
        CGContext::translate_ctm(self.c(), fitted.x, fitted.bottom());
        CGContext::scale_ctm(self.c(), 1.0, -1.0);
        CGContext::draw_image(
            self.c(),
            cg_rect(Rect::new(0.0, 0.0, fitted.width, fitted.height)),
            Some(vehicle),
        );
        CGContext::restore_g_state(self.c());
    }

    /// Draws `line` with its left edge at `x`, vertically centred in `band`.
    fn text(&self, line: &Line, x: f64, band: Rect, color: Rgba) {
        self.set_fill(color);
        let baseline = band.y + band.height / 2.0 + (line.ascent - line.descent) / 2.0;
        // With the flipped CTM, glyphs need a flipped text matrix to stay upright.
        CGContext::set_text_matrix(
            self.c(),
            CGAffineTransform {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: -1.0,
                tx: 0.0,
                ty: 0.0,
            },
        );
        CGContext::set_text_position(self.c(), x, baseline);
        unsafe { line.line.draw(self.ctx) };
    }
}

/// Runs `paint` on a transparent premultiplied RGBA context with a top-left origin.
fn draw(
    width: usize,
    height: usize,
    paint: impl FnOnce(&Canvas<'_>),
) -> Result<Drawn, OverlayError> {
    let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }))
        .ok_or(OverlayError::Graphics)?;
    let mut rgba = vec![0u8; width * height * 4];
    let ctx = unsafe {
        CGBitmapContextCreate(
            rgba.as_mut_ptr() as *mut c_void,
            width,
            height,
            8,
            width * 4,
            Some(&space),
            CGImageAlphaInfo::PremultipliedLast.0,
        )
    }
    .ok_or(OverlayError::Graphics)?;
    CGContext::translate_ctm(Some(&ctx), 0.0, height as f64);
    CGContext::scale_ctm(Some(&ctx), 1.0, -1.0);
    paint(&Canvas {
        ctx: &ctx,
        space: &space,
    });
    let image = CGBitmapContextCreateImage(Some(&ctx)).ok_or(OverlayError::Graphics)?;
    drop(ctx);
    Ok(Drawn {
        bitmap: Bitmap {
            width,
            height,
            rgba,
        },
        image,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn holes(layout: &ExportLayout) -> CameraMap<Rect> {
        layout.slots
    }

    fn rgb(pixel: [u8; 4]) -> [u8; 3] {
        [pixel[0], pixel[1], pixel[2]]
    }

    #[test]
    fn the_overlay_is_floor_outside_the_holes_and_transparent_inside() {
        let layout = ExportLayout::contract();
        let bitmap = render_overlay(&layout, &holes(&layout), VEHICLE_PNG).expect("overlay");
        assert_eq!((bitmap.width, bitmap.height), (1280, 720));
        // Far corner: plain floor, opaque.
        assert_eq!(bitmap.pixel(2, 2), [0xE9, 0xEB, 0xEE, 255]);
        assert_eq!(bitmap.pixel(1277, 717), [0xE9, 0xEB, 0xEE, 255]);
        for camera in CameraId::ALL {
            let slot = layout.slots[camera];
            let (cx, cy) = slot.center();
            assert_eq!(
                bitmap.pixel(cx as usize, cy as usize)[3],
                0,
                "{camera:?} hole centre"
            );
            // Just inside the straight edges: transparent; just outside: opaque floor.
            assert_eq!(
                bitmap.pixel(slot.x as usize + 1, cy as usize)[3],
                0,
                "{camera:?} left edge"
            );
            assert_eq!(
                bitmap.pixel(slot.x as usize - 2, cy as usize)[3],
                255,
                "{camera:?} outside left"
            );
            assert_eq!(
                bitmap.pixel(cx as usize, slot.y as usize + 1)[3],
                0,
                "{camera:?} top edge"
            );
            // The 12 px rounded corner keeps the very corner pixel covered.
            assert_eq!(
                bitmap.pixel(slot.x as usize + 1, slot.y as usize + 1)[3],
                255,
                "{camera:?} corner"
            );
            assert_eq!(
                bitmap.pixel(slot.right() as usize - 2, slot.bottom() as usize - 2)[3],
                255,
                "{camera:?} corner"
            );
        }
    }

    #[test]
    fn label_pills_sit_at_the_bottom_left_with_white_text() {
        let layout = ExportLayout::contract();
        let bitmap = render_overlay(&layout, &holes(&layout), VEHICLE_PNG).unwrap();
        let font = label_font(&layout);
        for camera in CameraId::ALL {
            let line = text_line(&layout.labels[camera], &font);
            let label = layout.label_geometry(layout.slots[camera], line.width);
            let pill = label.pill;
            // The pill's left padding (between the rounded end and the text) is pure scrim.
            let scrim = bitmap.pixel(
                (pill.x + 8.0) as usize,
                (pill.y + pill.height / 2.0) as usize,
            );
            assert_eq!(scrim[3], 0x8C, "{camera:?} scrim alpha");
            assert_eq!(rgb(scrim), [0, 0, 0], "{camera:?} scrim colour");
            // Above and left of the pill: still the transparent hole.
            assert_eq!(
                bitmap.pixel(
                    (pill.x + pill.width / 2.0) as usize,
                    (pill.y - 3.0) as usize
                )[3],
                0
            );
            assert_eq!(
                bitmap.pixel((pill.x - 3.0) as usize, (pill.y + 12.0) as usize)[3],
                0
            );
            // Text: some nearly white, opaque pixels inside the text box.
            let mut white = 0;
            for y in pill.y as usize + 4..(pill.bottom() as usize - 4) {
                for x in label.text_x as usize..(label.text_x + line.width) as usize {
                    let p = bitmap.pixel(x, y);
                    if p[3] == 255 && p[0] > 200 && p[1] > 200 && p[2] > 200 {
                        white += 1;
                    }
                }
            }
            assert!(white > 20, "{camera:?} label text pixels: {white}");
            assert!(
                pill.width < 120.0,
                "{camera:?} pill too wide: {}",
                pill.width
            );
        }
    }

    #[test]
    fn the_car_wedges_and_highlight_are_drawn_on_the_floor() {
        let layout = ExportLayout::contract();
        let bitmap = render_overlay(&layout, &holes(&layout), VEHICLE_PNG).unwrap();
        // The car body covers the middle of its box and is not floor-coloured.
        let (cx, cy) = layout.vehicle.center();
        let car = bitmap.pixel(cx as usize, cy as usize);
        assert_eq!(car[3], 255);
        assert_ne!(rgb(car), [0xE9, 0xEB, 0xEE]);
        // Near the car the floor is lighter (floor-light highlight); at the corners it is floor.
        let near = bitmap.pixel(440, 420);
        assert!(near[0] > 0xE9 + 3, "highlight {near:?}");
        // On the left pillar wedge's axis, the wedge darkens the floor compared with no wedges.
        let mut without = layout.clone();
        without.wedge.a = 0.0;
        let plain = render_overlay(&without, &holes(&layout), VEHICLE_PNG).unwrap();
        let (with_wedge, no_wedge) = (bitmap.pixel(550, 327), plain.pixel(550, 327));
        assert!(
            no_wedge[0] >= with_wedge[0] + 5,
            "wedge {with_wedge:?} vs {no_wedge:?}"
        );
        assert_eq!(
            bitmap.pixel(440, 420),
            plain.pixel(440, 420),
            "outside every wedge"
        );
    }

    #[test]
    fn a_broken_car_image_is_reported() {
        let layout = ExportLayout::contract();
        assert_eq!(
            render_overlay(&layout, &holes(&layout), b"not a png"),
            Err(OverlayError::VehicleImage)
        );
    }

    #[test]
    fn the_missing_tile_is_dark_with_a_light_centred_message() {
        let layout = ExportLayout::contract();
        let tile = draw_missing_tile(&layout, (352.0, 264.0)).unwrap().bitmap;
        assert_eq!((tile.width, tile.height), (352, 264));
        assert_eq!(tile.pixel(5, 5), [0x15, 0x18, 0x1D, 255]);
        let mut light = (usize::MAX, 0usize);
        for y in 120..144 {
            for x in 0..352 {
                if tile.pixel(x, y)[0] > 0x80 {
                    light = (light.0.min(x), light.1.max(x));
                }
            }
        }
        assert!(
            light.0 > 140 && light.1 < 212,
            "message not centred: {light:?}"
        );
    }
}
