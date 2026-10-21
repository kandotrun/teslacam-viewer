//! The design tokens of `DESIGN.md`, parsed from `packages/contract/design-tokens.json`.
//!
//! The JSON is embedded at build time and parsed strictly: an unknown or missing key fails
//! [`Theme::parse`], so a token added on the web side breaks the Mac build's tests until the Mac
//! theme knows it.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::OnceLock;

use serde::Deserialize;

use crate::camera::{CameraId, CameraMap};

/// The token file, embedded at build time.
pub const DESIGN_TOKENS_JSON: &str =
    include_str!("../../../../../packages/contract/design-tokens.json");

/// An sRGB colour with straight alpha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
    /// Alpha (255 = opaque).
    pub a: u8,
}

impl Rgba {
    /// Parses `#RRGGBB` or `#RRGGBBAA` (either case).
    pub fn parse(text: &str) -> Option<Rgba> {
        let hex = text.strip_prefix('#')?;
        if !(hex.len() == 6 || hex.len() == 8) || !hex.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return None;
        }
        let channel = |index: usize| u8::from_str_radix(&hex[index..index + 2], 16).ok();
        Some(Rgba {
            r: channel(0)?,
            g: channel(2)?,
            b: channel(4)?,
            a: if hex.len() == 8 { channel(6)? } else { 255 },
        })
    }

    /// Alpha as 0–1.
    pub fn alpha(self) -> f32 {
        f32::from(self.a) / 255.0
    }

    /// Whether the colour is fully opaque.
    pub fn is_opaque(self) -> bool {
        self.a == 255
    }

    /// This colour composited over `background` (source-over, straight alpha).
    pub fn over(self, background: Rgba) -> Rgba {
        let alpha = f64::from(self.a) / 255.0;
        let background_alpha = f64::from(background.a) / 255.0;
        let result_alpha = alpha + background_alpha * (1.0 - alpha);
        if result_alpha == 0.0 {
            return Rgba {
                r: 0,
                g: 0,
                b: 0,
                a: 0,
            };
        }
        let mix = |front: u8, back: u8| {
            ((f64::from(front) * alpha + f64::from(back) * background_alpha * (1.0 - alpha))
                / result_alpha)
                .round() as u8
        };
        Rgba {
            r: mix(self.r, background.r),
            g: mix(self.g, background.g),
            b: mix(self.b, background.b),
            a: (result_alpha * 255.0).round() as u8,
        }
    }

    /// WCAG relative luminance of the colour channels (alpha ignored).
    pub fn relative_luminance(self) -> f64 {
        let linear = |channel: u8| {
            let value = f64::from(channel) / 255.0;
            if value <= 0.039_28 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * linear(self.r) + 0.7152 * linear(self.g) + 0.0722 * linear(self.b)
    }

    /// WCAG contrast ratio of two opaque colours.
    pub fn contrast_ratio(self, other: Rgba) -> f64 {
        let (first, second) = (self.relative_luminance(), other.relative_luminance());
        (first.max(second) + 0.05) / (first.min(second) + 0.05)
    }
}

/// Generates the colour struct and its by-name lookup from one list of tokens.
macro_rules! colors {
    ($($field:ident => $token:literal: $doc:literal),* $(,)?) => {
        /// Every colour token of `DESIGN.md` §2.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct Colors {
            $(#[doc = $doc] pub $field: Rgba,)*
        }

        impl Colors {
            /// Token names in file order.
            pub const TOKENS: &'static [&'static str] = &[$($token),*];

            /// The colour of a token name such as `ink-2`.
            pub fn get(&self, token: &str) -> Option<Rgba> {
                match token {
                    $($token => Some(self.$field),)*
                    _ => None,
                }
            }

            fn from_map(mut map: BTreeMap<String, String>) -> Result<Colors, ThemeError> {
                let mut take = |token: &str| -> Result<Rgba, ThemeError> {
                    let value = map.remove(token).ok_or_else(|| ThemeError::new(format!("missing colour {token}")))?;
                    Rgba::parse(&value).ok_or_else(|| ThemeError::new(format!("invalid colour {token}: {value}")))
                };
                let colors = Colors { $($field: take($token)?,)* };
                match map.keys().next() {
                    Some(unknown) => Err(ThemeError::new(format!("unknown colour {unknown}"))),
                    None => Ok(colors),
                }
            }
        }
    };
}

colors! {
    floor => "floor": "App background.",
    floor_light => "floor-light": "Centre of the floor highlight behind the car.",
    surface => "surface": "Solid surfaces, selected segmented item, scrubber thumb.",
    glass => "glass": "Floating layers (no blur on the Mac).",
    ink => "ink": "Text, play button, hover rings.",
    ink_2 => "ink-2": "Secondary text.",
    ink_3 => "ink-3": "Tertiary text, axis labels.",
    line => "line": "The few separators that remain.",
    fill => "fill": "Segmented tracks, chips, icon buttons, scrubber track.",
    fill_hover => "fill-hover": "Hover on `fill` controls.",
    drive => "drive": "Primary action, selection, playhead, active wedge, focus ring.",
    drive_hover => "drive-hover": "Primary action hover.",
    drive_soft => "drive-soft": "Selected event row.",
    sentry => "sentry": "Sentry markers, detection marker.",
    sentry_ink => "sentry-ink": "Sentry text on `sentry-soft`.",
    sentry_soft => "sentry-soft": "Sentry chip background.",
    saved => "saved": "Saved markers.",
    saved_ink => "saved-ink": "Saved text on `saved-soft`.",
    saved_soft => "saved-soft": "Saved chip background.",
    recent => "recent": "Recent chip text.",
    recent_marker => "recent-marker": "Recent markers on the day rail.",
    warning => "warning": "Missing camera and scan warning text.",
    warning_soft => "warning-soft": "Warning background.",
    video => "video": "Video background and letterbox.",
    video_missing_a => "video-missing-a": "Stripes of a camera window without footage (A).",
    video_missing_b => "video-missing-b": "Stripes of a camera window without footage (B).",
    video_label_scrim => "video-label-scrim": "Camera name pill over video.",
    video_text => "video-text": "Text over video.",
    video_muted => "video-muted": "Secondary text over video.",
    scrim => "scrim": "Behind modal dialogs.",
    wedge => "wedge": "Field-of-view wedge, inactive (start colour; fades to transparent).",
    wedge_active => "wedge-active": "Field-of-view wedge of the active camera (start colour).",
}

/// One layer of a shadow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    /// Horizontal offset.
    pub x: f32,
    /// Vertical offset.
    pub y: f32,
    /// Blur radius.
    pub blur: f32,
    /// Colour.
    pub color: Rgba,
}

/// Shadows; only floating layers and the selected segmented item have one.
#[derive(Debug, Clone, PartialEq)]
pub struct Shadows {
    /// Floating layers.
    pub float: Vec<Shadow>,
    /// The selected segmented item.
    pub control: Vec<Shadow>,
}

/// Corner radii in points.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Radii {
    /// Floating panel and transport.
    pub panel: f32,
    /// Dialogs.
    pub dialog: f32,
    /// Camera windows.
    pub camera: f32,
    /// Event rows.
    pub row: f32,
    /// Segmented tracks.
    pub segmented: f32,
    /// Thumbnails.
    pub thumbnail: f32,
    /// Pills, chips and round buttons.
    pub pill: f32,
}

/// Font stacks.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fonts {
    /// UI text (resolves to Hiragino Sans for Japanese on the Mac).
    pub ui: Vec<String>,
    /// Numerals (Barlow, bundled).
    pub num: Vec<String>,
}

/// Which font stack a text style uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FontFamily {
    /// [`Fonts::ui`].
    Ui,
    /// [`Fonts::num`].
    Num,
}

/// One level of the type scale.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct TextStyle {
    /// Font stack.
    pub family: FontFamily,
    /// Weight (100–900).
    pub weight: u16,
    /// Size in points.
    pub size: f32,
    /// Line height as a multiple of the size.
    pub line_height: f32,
}

/// The type scale of `DESIGN.md` §3.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct TypeScale {
    /// Welcome headline.
    pub display: TextStyle,
    /// Day title in the library.
    pub title: TextStyle,
    /// Current playback time.
    pub clock: TextStyle,
    /// Event start time in the list.
    pub numeral_l: TextStyle,
    /// Time in the event pill, counts.
    pub numeral_m: TextStyle,
    /// Paragraphs.
    pub body: TextStyle,
    /// Buttons, pill text, details rows.
    pub ui: TextStyle,
    /// Facts, camera labels.
    pub small: TextStyle,
    /// Chips, rail hours, footnotes.
    pub micro: TextStyle,
}

/// Spacing in points.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spacing {
    /// Base unit.
    pub unit: f32,
    /// Gutter around floating layers.
    pub gutter: f32,
}

/// Fixed sizes in points.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Sizes {
    /// Library panel width.
    pub library_width: f32,
    /// Library panel width below 1280 pt.
    pub library_width_compact: f32,
    /// Event pill and actions pill height.
    pub topbar_height: f32,
    /// Transport maximum width.
    pub transport_max_width: f32,
    /// Round play button.
    pub play_button: f32,
    /// Event row thumbnail width (4:3).
    pub thumbnail_width: f32,
    /// Minimum hit target inside floating layers on desktop.
    pub hit_target_desktop: f32,
    /// Minimum hit target on touch layouts.
    pub hit_target_touch: f32,
}

/// One transition.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Transition {
    /// Duration (0 under "Reduce motion").
    pub duration_ms: u32,
    /// Cubic Bézier control points `x1, y1, x2, y2`.
    pub easing: [f32; 4],
}

/// Motion tokens.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Motion {
    /// Hover, pressed, focus, selection colour.
    pub fast: Transition,
    /// Expanding a camera, opening the details popover.
    pub sheet: Transition,
    /// Delay between camera windows filling with video after a folder loads.
    pub reveal_stagger_ms: u32,
}

/// Width breakpoints in points.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct Breakpoints {
    /// Phone layout up to this width (web only).
    pub phone_max: f32,
    /// Tablet layout up to this width (web only).
    pub tablet_max: f32,
    /// Compact library up to this width.
    pub compact_library_max: f32,
}

/// A cell of the vehicle layout grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StageArea {
    /// A camera window.
    Camera(CameraId),
    /// The car image.
    Vehicle,
}

/// The vehicle layout of the stage (`DESIGN.md` §5).
#[derive(Debug, Clone, PartialEq)]
pub struct StageGeometry {
    /// Grid areas, rows of three cells.
    pub areas: [[StageArea; 3]; 3],
    /// Row fractions.
    pub rows: [f32; 3],
    /// Column fractions.
    pub columns: [f32; 3],
    /// Gap between rows.
    pub row_gap: f32,
    /// Gap between columns.
    pub column_gap: f32,
    /// The back window's width as a fraction of its column.
    pub back_width_ratio: f32,
    /// Mount point of each camera relative to the car image box (`x`, `y` in 0–1).
    pub mounts: CameraMap<(f32, f32)>,
}

/// A colour pair that must reach a contrast minimum.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContrastPair {
    /// Foreground token.
    pub fg: String,
    /// Background token.
    pub bg: String,
    /// Minimum WCAG ratio.
    pub min: f64,
}

/// The parsed design tokens.
#[derive(Debug, Clone, PartialEq)]
pub struct Theme {
    /// Colours.
    pub colors: Colors,
    /// Shadows.
    pub shadows: Shadows,
    /// Radii.
    pub radii: Radii,
    /// Font stacks.
    pub fonts: Fonts,
    /// Type scale.
    pub type_scale: TypeScale,
    /// Spacing.
    pub spacing: Spacing,
    /// Sizes.
    pub sizes: Sizes,
    /// Motion.
    pub motion: Motion,
    /// Breakpoints.
    pub breakpoints: Breakpoints,
    /// Stage geometry.
    pub stage: StageGeometry,
    /// Contrast requirements.
    pub contrast: Vec<ContrastPair>,
}

/// The token file does not match the theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeError(String);

impl ThemeError {
    fn new(message: impl Into<String>) -> ThemeError {
        ThemeError(message.into())
    }
}

impl fmt::Display for ThemeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "design-tokens.json: {}", self.0)
    }
}

impl std::error::Error for ThemeError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawShadow {
    x: f32,
    y: f32,
    blur: f32,
    color: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawShadows {
    float: Vec<RawShadow>,
    control: Vec<RawShadow>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGap {
    row: f32,
    column: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct RawStage {
    areas: [[String; 3]; 3],
    rows: [f32; 3],
    columns: [f32; 3],
    gap: RawGap,
    back_width_ratio: f32,
    mounts: BTreeMap<String, (f32, f32)>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTheme {
    #[serde(rename = "$comment")]
    _comment: Option<String>,
    color: BTreeMap<String, String>,
    shadow: RawShadows,
    radius: Radii,
    font: Fonts,
    #[serde(rename = "type")]
    type_scale: TypeScale,
    space: Spacing,
    size: Sizes,
    motion: Motion,
    breakpoint: Breakpoints,
    stage: RawStage,
    contrast: Vec<ContrastPair>,
}

fn shadows(raw: Vec<RawShadow>) -> Result<Vec<Shadow>, ThemeError> {
    raw.into_iter()
        .map(|shadow| {
            Ok(Shadow {
                x: shadow.x,
                y: shadow.y,
                blur: shadow.blur,
                color: Rgba::parse(&shadow.color).ok_or_else(|| {
                    ThemeError::new(format!("invalid shadow colour {}", shadow.color))
                })?,
            })
        })
        .collect()
}

fn stage(raw: RawStage) -> Result<StageGeometry, ThemeError> {
    let area = |name: &String| match name.as_str() {
        "vehicle" => Ok(StageArea::Vehicle),
        other => CameraId::from_id(other)
            .map(StageArea::Camera)
            .ok_or_else(|| ThemeError::new(format!("unknown stage area {other}"))),
    };
    let mut areas = [[StageArea::Vehicle; 3]; 3];
    for (row, names) in areas.iter_mut().zip(&raw.areas) {
        for (cell, name) in row.iter_mut().zip(names) {
            *cell = area(name)?;
        }
    }
    let mut mounts = raw.mounts;
    let mut missing = None;
    let mount_map = CameraMap::from_fn(|camera| {
        mounts.remove(camera.id()).unwrap_or_else(|| {
            missing = Some(camera);
            (0.0, 0.0)
        })
    });
    if let Some(camera) = missing {
        return Err(ThemeError::new(format!("missing mount {}", camera.id())));
    }
    if let Some(unknown) = mounts.keys().next() {
        return Err(ThemeError::new(format!("unknown mount {unknown}")));
    }
    Ok(StageGeometry {
        areas,
        rows: raw.rows,
        columns: raw.columns,
        row_gap: raw.gap.row,
        column_gap: raw.gap.column,
        back_width_ratio: raw.back_width_ratio,
        mounts: mount_map,
    })
}

impl Theme {
    /// Parses a token file strictly.
    pub fn parse(json: &str) -> Result<Theme, ThemeError> {
        let raw: RawTheme =
            serde_json::from_str(json).map_err(|error| ThemeError::new(error.to_string()))?;
        let theme = Theme {
            colors: Colors::from_map(raw.color)?,
            shadows: Shadows {
                float: shadows(raw.shadow.float)?,
                control: shadows(raw.shadow.control)?,
            },
            radii: raw.radius,
            fonts: raw.font,
            type_scale: raw.type_scale,
            spacing: raw.space,
            sizes: raw.size,
            motion: raw.motion,
            breakpoints: raw.breakpoint,
            stage: stage(raw.stage)?,
            contrast: raw.contrast,
        };
        for pair in &theme.contrast {
            if theme.contrast_ratio(pair).is_none() {
                return Err(ThemeError::new(format!(
                    "unknown contrast token {} / {}",
                    pair.fg, pair.bg
                )));
            }
        }
        Ok(theme)
    }

    /// The embedded tokens.
    pub fn embedded() -> &'static Theme {
        static THEME: OnceLock<Theme> = OnceLock::new();
        THEME.get_or_init(|| {
            Theme::parse(DESIGN_TOKENS_JSON).expect("embedded design tokens are valid")
        })
    }

    /// The contrast of a pair. Translucent colours are composited over `surface` (white) first: it is
    /// the worst case for the dark scrims (white video under a label) and the backdrop of the soft
    /// chip colours.
    pub fn contrast_ratio(&self, pair: &ContrastPair) -> Option<f64> {
        let background = self.colors.get(&pair.bg)?.over(self.colors.surface);
        let foreground = self.colors.get(&pair.fg)?.over(background);
        Some(foreground.contrast_ratio(background))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_tokens_parse() {
        let theme = Theme::embedded();
        assert_eq!(
            theme.colors.floor,
            Rgba {
                r: 0xE9,
                g: 0xEB,
                b: 0xEE,
                a: 255
            }
        );
        assert_eq!(theme.colors.glass.a, 0xF0);
        assert_eq!(
            theme.colors.drive_soft,
            Rgba {
                r: 0x3E,
                g: 0x6A,
                b: 0xE1,
                a: 0x24
            }
        );
        assert_eq!(theme.colors.get("ink-3"), Some(theme.colors.ink_3));
        assert_eq!(theme.colors.get("nope"), None);
        assert_eq!(Colors::TOKENS.len(), 32);
        assert_eq!(theme.radii.camera, 14.0);
        assert_eq!(
            theme.type_scale.clock,
            TextStyle {
                family: FontFamily::Num,
                weight: 600,
                size: 34.0,
                line_height: 1.0
            }
        );
        assert_eq!(theme.fonts.num[0], "Barlow");
        assert_eq!(theme.sizes.library_width, 372.0);
        assert_eq!(theme.motion.sheet.duration_ms, 320);
        assert_eq!(theme.breakpoints.compact_library_max, 1280.0);
        assert_eq!(theme.shadows.float.len(), 2);
    }

    #[test]
    fn every_contrast_pair_meets_its_minimum() {
        let theme = Theme::embedded();
        assert!(!theme.contrast.is_empty());
        for pair in &theme.contrast {
            let ratio = theme.contrast_ratio(pair).expect("known tokens");
            assert!(
                ratio >= pair.min,
                "{} on {}: {ratio:.2} < {}",
                pair.fg,
                pair.bg,
                pair.min
            );
        }
    }

    #[test]
    fn the_stage_places_every_camera_once() {
        let stage = &Theme::embedded().stage;
        assert_eq!(stage.areas[1][1], StageArea::Vehicle);
        assert_eq!(stage.areas[0][1], StageArea::Camera(CameraId::Front));
        assert_eq!(stage.areas[2][1], StageArea::Camera(CameraId::Back));
        for camera in CameraId::ALL {
            assert!(
                stage
                    .areas
                    .iter()
                    .flatten()
                    .any(|area| *area == StageArea::Camera(camera)),
                "{camera:?}"
            );
        }
        assert_eq!(stage.mounts[CameraId::Back], (0.5, 0.93));
        assert_eq!((stage.row_gap, stage.column_gap), (14.0, 26.0));
    }

    #[test]
    fn parsing_is_strict() {
        let mut value: serde_json::Value = serde_json::from_str(DESIGN_TOKENS_JSON).expect("json");
        value["color"]["new-token"] = "#000000".into();
        assert!(Theme::parse(&value.to_string()).is_err());

        let mut value: serde_json::Value = serde_json::from_str(DESIGN_TOKENS_JSON).expect("json");
        value["color"]
            .as_object_mut()
            .expect("colors")
            .remove("ink");
        assert!(Theme::parse(&value.to_string()).is_err());

        let mut value: serde_json::Value = serde_json::from_str(DESIGN_TOKENS_JSON).expect("json");
        value["radius"]["extra"] = 1.into();
        assert!(Theme::parse(&value.to_string()).is_err());

        let mut value: serde_json::Value = serde_json::from_str(DESIGN_TOKENS_JSON).expect("json");
        value["color"]["ink"] = "rgb(0 0 0)".into();
        assert!(Theme::parse(&value.to_string()).is_err());

        let mut value: serde_json::Value = serde_json::from_str(DESIGN_TOKENS_JSON).expect("json");
        value["contrast"][0]["fg"] = "unknown".into();
        assert!(Theme::parse(&value.to_string()).is_err());
    }

    #[test]
    fn colour_math() {
        let white = Rgba::parse("#FFFFFF").expect("white");
        let black = Rgba::parse("#000000").expect("black");
        assert!((white.contrast_ratio(black) - 21.0).abs() < 1e-9);
        let scrim = Rgba::parse("#0000008C").expect("scrim");
        assert_eq!(
            scrim.over(white),
            Rgba {
                r: 115,
                g: 115,
                b: 115,
                a: 255
            }
        );
        assert_eq!(Rgba::parse("#abc"), None);
        assert_eq!(Rgba::parse("E9EBEE"), None);
        assert_eq!(Rgba::parse("#e9ebee"), Rgba::parse("#E9EBEE"));
        assert!((scrim.alpha() - 140.0 / 255.0).abs() < 1e-6);
        assert!(white.is_opaque() && !scrim.is_opaque());
    }
}
