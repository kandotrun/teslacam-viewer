//! The design tokens as GPUI values, and the small styling helpers every view shares.

use std::sync::Arc;

use gpui_kit::{
    App, BoxShadow, FontFeatures, FontWeight, Global, Hsla, IntoElement, ParentElement, Styled,
    Svg, div, point, px, relative, svg,
};
use teslacam_core::theme::{self as tokens, FontFamily, Shadow, TextStyle, Theme};

use crate::assets::{Icon, NUMERAL_FAMILY};
use crate::presentation::numerals::numeral_runs;

/// The UI font: Hiragino Sans first, like the web's stack (`DESIGN.md` §3), so Latin words inside
/// Japanese text use the same face and spacing as on the web.
pub const UI_FAMILY: &str = "Hiragino Sans";

/// A design token colour as a GPUI colour.
pub fn color(rgba: tokens::Rgba) -> Hsla {
    gpui_kit::Rgba {
        r: f32::from(rgba.r) / 255.0,
        g: f32::from(rgba.g) / 255.0,
        b: f32::from(rgba.b) / 255.0,
        a: f32::from(rgba.a) / 255.0,
    }
    .into()
}

/// Every colour token of `DESIGN.md` §2 (all of them, so a view never reaches for a raw value).
#[derive(Debug, Clone, Copy)]
#[allow(missing_docs, dead_code)]
pub struct Palette {
    pub floor: Hsla,
    pub floor_light: Hsla,
    pub surface: Hsla,
    pub glass: Hsla,
    pub ink: Hsla,
    pub ink_2: Hsla,
    pub ink_3: Hsla,
    pub line: Hsla,
    pub fill: Hsla,
    pub fill_hover: Hsla,
    pub drive: Hsla,
    pub drive_hover: Hsla,
    pub drive_soft: Hsla,
    pub sentry: Hsla,
    pub sentry_ink: Hsla,
    pub sentry_soft: Hsla,
    pub saved: Hsla,
    pub saved_ink: Hsla,
    pub saved_soft: Hsla,
    pub recent: Hsla,
    pub recent_marker: Hsla,
    pub warning: Hsla,
    pub warning_soft: Hsla,
    pub video: Hsla,
    pub video_missing_a: Hsla,
    pub video_missing_b: Hsla,
    pub video_label_scrim: Hsla,
    pub video_text: Hsla,
    pub video_muted: Hsla,
    pub scrim: Hsla,
    pub wedge: Hsla,
    pub wedge_active: Hsla,
}

impl Palette {
    fn new(colors: &tokens::Colors) -> Palette {
        Palette {
            floor: color(colors.floor),
            floor_light: color(colors.floor_light),
            surface: color(colors.surface),
            glass: color(colors.glass),
            ink: color(colors.ink),
            ink_2: color(colors.ink_2),
            ink_3: color(colors.ink_3),
            line: color(colors.line),
            fill: color(colors.fill),
            fill_hover: color(colors.fill_hover),
            drive: color(colors.drive),
            drive_hover: color(colors.drive_hover),
            drive_soft: color(colors.drive_soft),
            sentry: color(colors.sentry),
            sentry_ink: color(colors.sentry_ink),
            sentry_soft: color(colors.sentry_soft),
            saved: color(colors.saved),
            saved_ink: color(colors.saved_ink),
            saved_soft: color(colors.saved_soft),
            recent: color(colors.recent),
            recent_marker: color(colors.recent_marker),
            warning: color(colors.warning),
            warning_soft: color(colors.warning_soft),
            video: color(colors.video),
            video_missing_a: color(colors.video_missing_a),
            video_missing_b: color(colors.video_missing_b),
            video_label_scrim: color(colors.video_label_scrim),
            video_text: color(colors.video_text),
            video_muted: color(colors.video_muted),
            scrim: color(colors.scrim),
            wedge: color(colors.wedge),
            wedge_active: color(colors.wedge_active),
        }
    }
}

/// The app's theme: the parsed tokens and their GPUI colours.
pub struct AppTheme {
    /// `design-tokens.json`.
    pub tokens: &'static Theme,
    /// Colours.
    pub palette: Palette,
    /// `shadow-float`.
    pub float_shadow: Vec<BoxShadow>,
    /// `shadow-control`.
    pub control_shadow: Vec<BoxShadow>,
}

impl Global for AppTheme {}

fn shadows(layers: &[Shadow]) -> Vec<BoxShadow> {
    layers
        .iter()
        .map(|layer| BoxShadow {
            color: color(layer.color),
            offset: point(px(layer.x), px(layer.y)),
            // GPUI's blur radius means the same as CSS's (sigma = radius / 2).
            blur_radius: px(layer.blur),
            spread_radius: px(0.0),
            inset: false,
        })
        .collect()
}

impl AppTheme {
    /// Installs the embedded tokens as a global.
    pub fn install(cx: &mut App) {
        let tokens = Theme::embedded();
        cx.set_global(AppTheme {
            tokens,
            palette: Palette::new(&tokens.colors),
            float_shadow: shadows(&tokens.shadows.float),
            control_shadow: shadows(&tokens.shadows.control),
        });
    }

    /// The installed theme.
    pub fn get(cx: &App) -> &AppTheme {
        cx.global::<AppTheme>()
    }
}

/// Tabular figures for the numeral font.
pub fn tabular() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

/// Text style helpers.
pub trait TypeExt: Styled + Sized {
    /// Applies a type role of `DESIGN.md` §3 (family, weight, size, line height).
    fn type_role(self, role: &TextStyle) -> Self {
        let styled = self
            .text_size(px(role.size))
            .font_weight(FontWeight(f32::from(role.weight)))
            .line_height(relative(role.line_height));
        match role.family {
            FontFamily::Num => styled.numeral(),
            FontFamily::Ui => styled.font_family(UI_FAMILY),
        }
    }

    /// Barlow with tabular figures.
    fn numeral(self) -> Self {
        self.font_family(NUMERAL_FAMILY).font_features(tabular())
    }

    /// The UI font at `size` and `weight`.
    fn ui_text(self, size: f32, weight: f32) -> Self {
        self.font_family(UI_FAMILY)
            .text_size(px(size))
            .font_weight(FontWeight(weight))
    }
}

impl<T: Styled> TypeExt for T {}

/// An icon at `size` points in `color`.
pub fn icon(icon: Icon, size: f32, color: Hsla) -> Svg {
    svg()
        .path(icon.path())
        .size(px(size))
        .flex_none()
        .text_color(color)
}

/// Text whose digit runs are set in Barlow (`numeral_size`, `numeral_weight`, `numeral_color`) and
/// the rest in the inherited UI style: `2分` → **2**分.
pub fn numeral_text(
    text: &str,
    numeral_size: f32,
    numeral_weight: f32,
    numeral_color: Option<Hsla>,
) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_baseline()
        .children(numeral_runs(text).into_iter().map(|run| {
            let piece = div().child(run.text.to_owned());
            if run.numeral {
                let piece = piece
                    .numeral()
                    .text_size(px(numeral_size))
                    .font_weight(FontWeight(numeral_weight));
                match numeral_color {
                    Some(color) => piece.text_color(color),
                    None => piece,
                }
            } else {
                piece
            }
        }))
}
