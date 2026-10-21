//! Geometry and colours of the 1280 × 720 export frame (DESIGN.md "Export frame").
//!
//! Pure logic. Every value comes from `packages/contract/composite-layout.json` and
//! `packages/contract/design-tokens.json`, compiled in with `include_str!`, so the Mac export cannot
//! drift from the shared contract. Coordinates are pixels with the origin at the top-left.

use serde::Deserialize;
use std::collections::BTreeMap;

use crate::input::{CameraId, CameraMap};

const LAYOUT_JSON: &str = include_str!("../../../../../packages/contract/composite-layout.json");
const TOKENS_JSON: &str = include_str!("../../../../../packages/contract/design-tokens.json");

/// An axis-aligned rectangle (top-left origin).
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct Rect {
    /// Left edge.
    pub x: f64,
    /// Top edge.
    pub y: f64,
    /// Width.
    pub width: f64,
    /// Height.
    pub height: f64,
}

impl Rect {
    /// A rectangle from its top-left corner and size.
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    /// Right edge.
    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    /// Bottom edge.
    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    /// Centre point.
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

/// A straight (non-premultiplied) colour with components in 0…1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    /// Red.
    pub r: f64,
    /// Green.
    pub g: f64,
    /// Blue.
    pub b: f64,
    /// Alpha.
    pub a: f64,
}

impl Rgba {
    /// Parses `#RRGGBB` or `#RRGGBBAA`.
    pub fn parse_hex(hex: &str) -> Option<Rgba> {
        let digits = hex.strip_prefix('#')?;
        if !matches!(digits.len(), 6 | 8) || !digits.is_ascii() {
            return None;
        }
        let channel = |i: usize| {
            u8::from_str_radix(&digits[i..i + 2], 16)
                .ok()
                .map(|v| f64::from(v) / 255.0)
        };
        let a = if digits.len() == 8 { channel(6)? } else { 1.0 };
        Some(Rgba {
            r: channel(0)?,
            g: channel(2)?,
            b: channel(4)?,
            a,
        })
    }

    /// The colour as 8-bit sRGB channels `[r, g, b]` (alpha ignored).
    pub fn rgb8(&self) -> [u8; 3] {
        [self.r, self.g, self.b].map(|c| (c * 255.0).round() as u8)
    }
}

/// Fits `source` inside `destination`, centred, preserving its aspect ratio (spec §3.3; no rounding).
/// A degenerate source returns the destination.
pub fn contain_rect(source_width: f64, source_height: f64, destination: Rect) -> Rect {
    if source_width <= 0.0 || source_height <= 0.0 {
        return destination;
    }
    let scale = (destination.width / source_width).min(destination.height / source_height);
    let (width, height) = (source_width * scale, source_height * scale);
    Rect::new(
        destination.x + (destination.width - width) / 2.0,
        destination.y + (destination.height - height) / 2.0,
        width,
        height,
    )
}

/// A 2D affine transform `[a, b, c, d, tx, ty]` (CoreGraphics convention:
/// `x' = a·x + c·y + tx`, `y' = b·x + d·y + ty`).
pub type Affine = [f64; 6];

/// Size of a video frame of `natural` size once its preferred transform is applied.
pub fn display_size(natural: (f64, f64), preferred: Affine) -> (f64, f64) {
    let (min_x, min_y, max_x, max_y) = transformed_bounds(natural, preferred);
    (max_x - min_x, max_y - min_y)
}

/// The transform that draws a frame of `natural` size (with its `preferred` transform) contain-fitted
/// into `destination`.
pub fn fit_transform(natural: (f64, f64), preferred: Affine, destination: Rect) -> Affine {
    let (min_x, min_y, max_x, max_y) = transformed_bounds(natural, preferred);
    let fitted = contain_rect(max_x - min_x, max_y - min_y, destination);
    let scale = if max_x > min_x {
        fitted.width / (max_x - min_x)
    } else {
        1.0
    };
    let [a, b, c, d, tx, ty] = preferred;
    [
        scale * a,
        scale * b,
        scale * c,
        scale * d,
        scale * (tx - min_x) + fitted.x,
        scale * (ty - min_y) + fitted.y,
    ]
}

fn transformed_bounds(natural: (f64, f64), t: Affine) -> (f64, f64, f64, f64) {
    let (w, h) = natural;
    let corners = [(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)]
        .map(|(x, y)| (t[0] * x + t[2] * y + t[4], t[1] * x + t[3] * y + t[5]));
    corners.iter().fold(
        (
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ),
        |(a, b, c, d), &(x, y)| (a.min(x), b.min(y), c.max(x), d.max(y)),
    )
}

/// Label pill style (`composite-layout.json#/label`).
#[derive(Debug, Clone, PartialEq)]
pub struct LabelStyle {
    /// Distance from the fitted video rect's left and bottom edges.
    pub inset: f64,
    /// Pill height.
    pub height: f64,
    /// Horizontal padding inside the pill.
    pub padding_inline: f64,
    /// Font size in pixels.
    pub font_size: f64,
    /// CSS font weight (500 = medium).
    pub font_weight: u16,
    /// Pill fill.
    pub scrim: Rgba,
    /// Text colour.
    pub text: Rgba,
}

/// Missing tile style (`composite-layout.json#/missing`).
#[derive(Debug, Clone, PartialEq)]
pub struct MissingStyle {
    /// Tile fill.
    pub fill: Rgba,
    /// Message colour.
    pub text: Rgba,
    /// The centred message (`記録なし`).
    pub message: String,
}

/// Where a label pill and its text go.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LabelGeometry {
    /// The pill (fully rounded ends).
    pub pill: Rect,
    /// X of the text's left edge.
    pub text_x: f64,
}

/// A field-of-view wedge: a triangle from the mount point to the two window corners that face the
/// car, filled with a linear gradient from `gradient.0` (wedge colour) to `gradient.1` (transparent).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wedge {
    /// Mount point, then the two near corners.
    pub points: [(f64, f64); 3],
    /// Gradient axis: mount point → midpoint of the near edge.
    pub gradient: ((f64, f64), (f64, f64)),
}

/// The complete export frame description.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportLayout {
    /// Frame width (1280).
    pub width: f64,
    /// Frame height (720).
    pub height: f64,
    /// Output frame rate (30).
    pub fps: u32,
    /// Slot per camera; video is contain-fitted inside it.
    pub slots: CameraMap<Rect>,
    /// Box of the car image.
    pub vehicle: Rect,
    /// Corner radius of the fitted video rects.
    pub video_corner_radius: f64,
    /// Label pills.
    pub label: LabelStyle,
    /// Missing tiles.
    pub missing: MissingStyle,
    /// Inactive wedge colour (its alpha is the gradient's start opacity).
    pub wedge: Rgba,
    /// Mount points relative to the car image box (0…1).
    pub mounts: CameraMap<(f64, f64)>,
    /// Japanese camera names.
    pub labels: CameraMap<String>,
    /// Background colour.
    pub floor: Rgba,
    /// Centre of the floor highlight behind the car.
    pub floor_light: Rgba,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LayoutFile {
    width: f64,
    height: f64,
    fps: u32,
    background: String,
    slots: BTreeMap<String, Rect>,
    vehicle: Rect,
    video_corner_radius: f64,
    label: LabelFile,
    missing: MissingFile,
    wedges: WedgesFile,
    labels: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LabelFile {
    inset: f64,
    height: f64,
    padding_inline: f64,
    font_size: f64,
    font_weight: u16,
    scrim: String,
    text: String,
}

#[derive(Deserialize)]
struct MissingFile {
    fill: String,
    text: String,
    message: String,
}

#[derive(Deserialize)]
struct WedgesFile {
    color: String,
}

#[derive(Deserialize)]
struct TokensFile {
    color: BTreeMap<String, String>,
    stage: StageFile,
}

#[derive(Deserialize)]
struct StageFile {
    mounts: BTreeMap<String, (f64, f64)>,
}

impl ExportLayout {
    /// The layout of the shared contract compiled into this crate.
    pub fn contract() -> ExportLayout {
        ExportLayout::parse(LAYOUT_JSON, TOKENS_JSON)
            .expect("packages/contract export layout is valid")
    }

    /// Parses `composite-layout.json` with colours and mounts resolved from `design-tokens.json`.
    pub fn parse(layout_json: &str, tokens_json: &str) -> Result<ExportLayout, String> {
        let layout: LayoutFile = serde_json::from_str(layout_json).map_err(|e| e.to_string())?;
        let tokens: TokensFile = serde_json::from_str(tokens_json).map_err(|e| e.to_string())?;
        let color = |name: &str| {
            tokens
                .color
                .get(name)
                .and_then(|hex| Rgba::parse_hex(hex))
                .ok_or_else(|| format!("unknown colour token `{name}`"))
        };
        let per_camera = |what: &str, lookup: &dyn Fn(&str) -> Option<()>| {
            CameraId::ALL
                .iter()
                .find(|camera| lookup(camera.id()).is_none())
                .map_or(Ok(()), |camera| {
                    Err(format!("{what} has no entry for `{}`", camera.id()))
                })
        };
        per_camera("slots", &|id| layout.slots.get(id).map(|_| ()))?;
        per_camera("labels", &|id| layout.labels.get(id).map(|_| ()))?;
        per_camera("mounts", &|id| tokens.stage.mounts.get(id).map(|_| ()))?;
        Ok(ExportLayout {
            width: layout.width,
            height: layout.height,
            fps: layout.fps,
            slots: CameraMap::from_fn(|camera| layout.slots[camera.id()]),
            vehicle: layout.vehicle,
            video_corner_radius: layout.video_corner_radius,
            label: LabelStyle {
                inset: layout.label.inset,
                height: layout.label.height,
                padding_inline: layout.label.padding_inline,
                font_size: layout.label.font_size,
                font_weight: layout.label.font_weight,
                scrim: color(&layout.label.scrim)?,
                text: color(&layout.label.text)?,
            },
            missing: MissingStyle {
                fill: color(&layout.missing.fill)?,
                text: color(&layout.missing.text)?,
                message: layout.missing.message,
            },
            wedge: color(&layout.wedges.color)?,
            mounts: CameraMap::from_fn(|camera| tokens.stage.mounts[camera.id()]),
            labels: CameraMap::from_fn(|camera| layout.labels[camera.id()].clone()),
            floor: color(&layout.background)?,
            floor_light: color("floor-light")?,
        })
    }

    /// The fitted video rect of `camera` for a source of the given display size (the slot itself when
    /// the size is unknown).
    pub fn video_rect(&self, camera: CameraId, display_size: Option<(f64, f64)>) -> Rect {
        let slot = self.slots[camera];
        display_size.map_or(slot, |(width, height)| contain_rect(width, height, slot))
    }

    /// Absolute position of `camera`'s mount point on the car.
    pub fn mount_point(&self, camera: CameraId) -> (f64, f64) {
        let (fx, fy) = self.mounts[camera];
        (
            self.vehicle.x + fx * self.vehicle.width,
            self.vehicle.y + fy * self.vehicle.height,
        )
    }

    /// The label pill at the bottom-left of a fitted video rect, for a measured text advance width.
    pub fn label_geometry(&self, video: Rect, text_width: f64) -> LabelGeometry {
        let style = &self.label;
        let pill = Rect::new(
            video.x + style.inset,
            video.bottom() - style.inset - style.height,
            text_width.ceil() + 2.0 * style.padding_inline,
            style.height,
        );
        LabelGeometry {
            pill,
            text_x: pill.x + style.padding_inline,
        }
    }

    /// The wedge from `camera`'s mount point to the near edge of its window (`window` = fitted rect).
    /// Matches the prototype: side edges are inset 10 px vertically, top/bottom edges 14 px horizontally.
    pub fn wedge(&self, camera: CameraId, window: Rect) -> Wedge {
        let (mx, my) = self.mount_point(camera);
        let (cx, cy) = window.center();
        let (a, b) = if (cx - mx).abs() > (cy - my).abs() {
            let x = if cx < mx { window.right() } else { window.x };
            ((x, window.y + 10.0), (x, window.bottom() - 10.0))
        } else {
            let y = if cy < my { window.bottom() } else { window.y };
            ((window.x + 14.0, y), (window.right() - 14.0, y))
        };
        let mid = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
        Wedge {
            points: [(mx, my), a, b],
            gradient: ((mx, my), mid),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> ExportLayout {
        ExportLayout::contract()
    }

    #[test]
    fn contract_slots_match_design_md() {
        let layout = layout();
        assert_eq!(
            (layout.width, layout.height, layout.fps),
            (1280.0, 720.0, 30)
        );
        assert_eq!(
            layout.slots[CameraId::Front],
            Rect::new(464.0, 12.0, 352.0, 264.0)
        );
        assert_eq!(
            layout.slots[CameraId::LeftPillar],
            Rect::new(56.0, 112.0, 352.0, 264.0)
        );
        assert_eq!(
            layout.slots[CameraId::RightPillar],
            Rect::new(872.0, 112.0, 352.0, 264.0)
        );
        assert_eq!(
            layout.slots[CameraId::LeftRepeater],
            Rect::new(56.0, 444.0, 352.0, 264.0)
        );
        assert_eq!(
            layout.slots[CameraId::RightRepeater],
            Rect::new(872.0, 444.0, 352.0, 264.0)
        );
        assert_eq!(
            layout.slots[CameraId::Back],
            Rect::new(480.0, 456.0, 320.0, 240.0)
        );
        assert_eq!(layout.vehicle, Rect::new(598.0, 285.0, 84.0, 168.0));
        assert_eq!(layout.video_corner_radius, 12.0);
    }

    #[test]
    fn contract_styles_resolve_design_tokens() {
        let layout = layout();
        assert_eq!(layout.floor.rgb8(), [0xE9, 0xEB, 0xEE]);
        assert_eq!(layout.floor_light.rgb8(), [0xF7, 0xF8, 0xFA]);
        assert_eq!(layout.label.scrim.rgb8(), [0, 0, 0]);
        assert!((layout.label.scrim.a - 0x8C as f64 / 255.0).abs() < 1e-9);
        assert_eq!(layout.label.text.rgb8(), [255, 255, 255]);
        assert_eq!(
            (
                layout.label.inset,
                layout.label.height,
                layout.label.padding_inline
            ),
            (10.0, 24.0, 10.0)
        );
        assert_eq!(
            (layout.label.font_size, layout.label.font_weight),
            (13.0, 500)
        );
        assert_eq!(layout.missing.fill.rgb8(), [0x15, 0x18, 0x1D]);
        assert_eq!(layout.missing.text.rgb8(), [0xC2, 0xC8, 0xD0]);
        assert_eq!(layout.missing.message, "記録なし");
        assert_eq!(layout.wedge.rgb8(), [0x17, 0x1A, 0x20]);
        assert!((layout.wedge.a - 0x12 as f64 / 255.0).abs() < 1e-9);
    }

    #[test]
    fn contract_labels_are_the_japanese_camera_names() {
        let layout = layout();
        let labels: Vec<_> = CameraId::ALL
            .iter()
            .map(|c| layout.labels[*c].as_str())
            .collect();
        assert_eq!(
            labels,
            [
                "フロント",
                "リア",
                "左ピラー",
                "左リピーター",
                "右ピラー",
                "右リピーター"
            ]
        );
        for camera in CameraId::ALL {
            assert_eq!(layout.labels[camera], camera.label(), "{camera:?}");
        }
    }

    #[test]
    fn hex_colours_parse_with_and_without_alpha() {
        assert_eq!(
            Rgba::parse_hex("#FF0000"),
            Some(Rgba {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0
            })
        );
        assert_eq!(Rgba::parse_hex("#00000000").map(|c| c.a), Some(0.0));
        assert_eq!(Rgba::parse_hex("FF0000"), None);
        assert_eq!(Rgba::parse_hex("#FF00"), None);
        assert_eq!(Rgba::parse_hex("#GG0000"), None);
    }

    #[test]
    fn contain_fit_matches_the_spec_examples() {
        let front = Rect::new(400.0, 12.0, 480.0, 216.0);
        let fitted = contain_rect(1448.0, 938.0, front);
        assert!((fitted.x - 473.279_317).abs() < 1e-3 && (fitted.width - 333.441_365).abs() < 1e-3);
        assert_eq!(
            contain_rect(320.0, 180.0, front),
            Rect::new(448.0, 12.0, 384.0, 216.0)
        );
        assert_eq!(
            contain_rect(1920.0, 1080.0, Rect::new(0.0, 0.0, 300.0, 300.0)),
            Rect::new(0.0, 65.625, 300.0, 168.75)
        );
        assert_eq!(contain_rect(0.0, 10.0, front), front);
    }

    #[test]
    fn four_by_three_cameras_fill_their_slots_and_the_car_fills_its_box() {
        let layout = layout();
        for camera in CameraId::ALL {
            assert_eq!(
                layout.video_rect(camera, Some((1280.0, 960.0))),
                layout.slots[camera]
            );
        }
        assert_eq!(contain_rect(360.0, 720.0, layout.vehicle), layout.vehicle);
        let wide = layout.video_rect(CameraId::Front, Some((1920.0, 1080.0)));
        assert_eq!(wide, Rect::new(464.0, 45.0, 352.0, 198.0));
        assert_eq!(
            layout.video_rect(CameraId::Back, None),
            layout.slots[CameraId::Back]
        );
    }

    const IDENTITY: Affine = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

    fn apply(t: Affine, (x, y): (f64, f64)) -> (f64, f64) {
        (t[0] * x + t[2] * y + t[4], t[1] * x + t[3] * y + t[5])
    }

    #[test]
    fn fit_transform_scales_an_upright_frame_into_its_slot() {
        let slot = Rect::new(56.0, 112.0, 352.0, 264.0);
        let t = fit_transform((1280.0, 960.0), IDENTITY, slot);
        assert_eq!(t, [0.275, 0.0, 0.0, 0.275, 56.0, 112.0]);
        assert_eq!(display_size((1280.0, 960.0), IDENTITY), (1280.0, 960.0));
    }

    #[test]
    fn fit_transform_applies_the_preferred_rotation_first() {
        // A portrait clip stored landscape with a 90° preferred transform.
        let rotate = [0.0, 1.0, -1.0, 0.0, 960.0, 0.0];
        assert_eq!(display_size((1280.0, 960.0), rotate), (960.0, 1280.0));
        let slot = Rect::new(0.0, 0.0, 300.0, 300.0);
        let t = fit_transform((1280.0, 960.0), rotate, slot);
        let fitted = contain_rect(960.0, 1280.0, slot);
        let corners = [(0.0, 0.0), (1280.0, 960.0)].map(|p| apply(t, p));
        let (xs, ys): (Vec<f64>, Vec<f64>) = corners.iter().copied().unzip();
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(close(
            xs.iter().cloned().fold(f64::INFINITY, f64::min),
            fitted.x
        ));
        assert!(close(
            xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            fitted.right()
        ));
        assert!(close(
            ys.iter().cloned().fold(f64::INFINITY, f64::min),
            fitted.y
        ));
        assert!(close(
            ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            fitted.bottom()
        ));
    }

    #[test]
    fn label_pills_sit_at_the_bottom_left_of_the_video() {
        let layout = layout();
        let back = layout.slots[CameraId::Back];
        let label = layout.label_geometry(back, 25.6);
        assert_eq!(label.pill, Rect::new(490.0, 662.0, 46.0, 24.0));
        assert_eq!(label.text_x, 500.0);
    }

    #[test]
    fn mount_points_are_relative_to_the_car_box() {
        let layout = layout();
        let (x, y) = layout.mount_point(CameraId::Front);
        assert!((x - 640.0).abs() < 1e-9 && (y - 321.96).abs() < 1e-9);
        let (x, y) = layout.mount_point(CameraId::Back);
        assert!((x - 640.0).abs() < 1e-9 && (y - 441.24).abs() < 1e-9);
    }

    #[test]
    fn wedges_reach_the_window_edge_that_faces_the_car() {
        let layout = layout();
        let front = layout.wedge(CameraId::Front, layout.slots[CameraId::Front]);
        assert_eq!(
            (front.points[1], front.points[2]),
            ((478.0, 276.0), (802.0, 276.0))
        );
        assert_eq!(front.gradient.1, (640.0, 276.0));
        let left = layout.wedge(CameraId::LeftPillar, layout.slots[CameraId::LeftPillar]);
        assert_eq!(
            (left.points[1], left.points[2]),
            ((408.0, 122.0), (408.0, 366.0))
        );
        let right = layout.wedge(
            CameraId::RightRepeater,
            layout.slots[CameraId::RightRepeater],
        );
        assert_eq!(
            (right.points[1], right.points[2]),
            ((872.0, 454.0), (872.0, 698.0))
        );
        let back = layout.wedge(CameraId::Back, layout.slots[CameraId::Back]);
        assert_eq!(
            (back.points[1], back.points[2]),
            ((494.0, 456.0), (786.0, 456.0))
        );
    }

    #[test]
    fn malformed_contracts_are_reported() {
        assert!(ExportLayout::parse("{}", TOKENS_JSON).is_err());
        let without_back = LAYOUT_JSON.replace("\"back\": { \"x\": 480", "\"rear\": { \"x\": 480");
        assert_eq!(
            ExportLayout::parse(&without_back, TOKENS_JSON).unwrap_err(),
            "slots has no entry for `back`"
        );
    }
}
