//! The two soft gradients of the garage floor, as small SVG documents that GPUI rasterises once and
//! stretches over their boxes (GPUI has linear gradients only).
//!
//! - The floor: `radial-gradient(ellipse 70% 75% at 62% 46%, floor-light 0%, floor 62%)` over the
//!   whole window (prototype `body` background).
//! - The car's floor shadow: an ellipse `inset: 6% -18% 2%` around the car image with
//!   `radial-gradient(closest-side, ink 22%, transparent)`.

use teslacam_core::camera::{CameraId, CameraMap};
use teslacam_core::theme::Rgba;

use super::stage_layout::Wedge;

fn hex(color: Rgba) -> String {
    format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b)
}

/// The floor background. The document is a 100 × 100 square meant to be stretched to the window
/// (`ObjectFit::Fill`), so percentages map like CSS.
pub fn floor_svg(floor: Rgba, floor_light: Rgba) -> String {
    // The ellipse's radii are 70 % of the width and 75 % of the height; userSpaceOnUse in the
    // 100 × 100 box with a vertical stretch of 75 / 70 about the centre.
    format!(
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 100 100" preserveAspectRatio="none">"#,
            r#"<defs><radialGradient id="f" gradientUnits="userSpaceOnUse" cx="62" cy="46" r="70" "#,
            r#"gradientTransform="translate(62 46) scale(1 {stretch}) translate(-62 -46)">"#,
            r#"<stop offset="0" stop-color="{light}"/><stop offset="0.62" stop-color="{floor}"/>"#,
            r#"<stop offset="1" stop-color="{floor}"/></radialGradient></defs>"#,
            r#"<rect width="100" height="100" fill="url(#f)"/></svg>"#
        ),
        stretch = 75.0 / 70.0,
        light = hex(floor_light),
        floor = hex(floor),
    )
}

/// Where the car's floor shadow sits relative to the car image box `(x, y, width, height)` as
/// fractions: 18 % wider on each side, 6 % below the top, 2 % above the bottom.
pub const CAR_SHADOW_BOX: (f32, f32, f32, f32) = (-0.18, 0.06, 1.36, 0.92);

/// The car's floor shadow, meant to be stretched to [`CAR_SHADOW_BOX`].
pub fn car_shadow_svg(ink: Rgba) -> String {
    format!(
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="272" height="368" viewBox="0 0 136 184" preserveAspectRatio="none">"#,
            r#"<defs><radialGradient id="s" cx="0.5" cy="0.5" r="0.5">"#,
            r#"<stop offset="0" stop-color="{ink}" stop-opacity="0.22"/>"#,
            r#"<stop offset="1" stop-color="{ink}" stop-opacity="0"/></radialGradient></defs>"#,
            r#"<ellipse cx="68" cy="92" rx="68" ry="92" fill="url(#s)"/></svg>"#
        ),
        ink = hex(ink),
    )
}

/// The 20 pt progress ring of a running export: a `track` circle and the done part in `bar`,
/// starting at twelve o'clock (web `ProgressRing`).
pub fn progress_ring_svg(progress: f32, track: Rgba, bar: Rgba) -> String {
    let circumference = 2.0 * std::f32::consts::PI * 8.0;
    let offset = circumference * (1.0 - progress.clamp(0.0, 1.0));
    format!(
        concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40" viewBox="0 0 20 20" fill="none" stroke-width="2.5">"#,
            r#"<circle cx="10" cy="10" r="8" stroke="{track}"/>"#,
            r#"<circle cx="10" cy="10" r="8" stroke="{bar}" stroke-linecap="round" "#,
            r#"stroke-dasharray="{circumference:.2}" stroke-dashoffset="{offset:.2}" transform="rotate(-90 10 10)"/></svg>"#
        ),
        track = hex(track),
        bar = hex(bar),
        circumference = circumference,
        offset = offset,
    )
}

/// The field-of-view wedges of a `width` × `height` stage as one SVG document, like the web's
/// stage overlay: each triangle runs from its mount point to the window corners that face the car,
/// filled with a gradient along mount → edge midpoint from the wedge colour (its own alpha times
/// `opacity`) to transparent. `active` uses the `wedge-active` colour.
pub fn wedges_svg(
    width: f32,
    height: f32,
    wedges: &CameraMap<Wedge>,
    active: Option<CameraId>,
    (wedge, wedge_active): (Rgba, Rgba),
    opacity: f32,
) -> String {
    let mut defs = String::new();
    let mut shapes = String::new();
    for (camera, wedge_geometry) in wedges.iter() {
        let color = if Some(camera) == active {
            wedge_active
        } else {
            wedge
        };
        let alpha = f32::from(color.a) / 255.0 * opacity;
        let id = camera.id();
        let (mx, my) = wedge_geometry.mount;
        let (ex, ey) = wedge_geometry.edge_midpoint;
        defs.push_str(&format!(
            r#"<linearGradient id="{id}" gradientUnits="userSpaceOnUse" x1="{mx:.2}" y1="{my:.2}" x2="{ex:.2}" y2="{ey:.2}"><stop offset="0" stop-color="{hex}" stop-opacity="{alpha:.4}"/><stop offset="1" stop-color="{hex}" stop-opacity="0"/></linearGradient>"#,
            hex = hex(color),
        ));
        let [(ax, ay), (bx, by)] = wedge_geometry.corners;
        shapes.push_str(&format!(
            r#"<polygon points="{mx:.2},{my:.2} {ax:.2},{ay:.2} {bx:.2},{by:.2}" fill="url(#{id})"/>"#
        ));
    }
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.2} {height:.2}"><defs>{defs}</defs>{shapes}</svg>"#
    )
}

#[cfg(test)]
mod tests {
    use teslacam_core::theme::Theme;

    use super::*;

    #[test]
    fn the_floor_uses_the_floor_tokens() {
        let colors = &Theme::embedded().colors;
        let svg = floor_svg(colors.floor, colors.floor_light);
        assert!(
            svg.contains(r##"<stop offset="0" stop-color="#F7F8FA"/>"##),
            "{svg}"
        );
        assert!(
            svg.contains(r##"<stop offset="0.62" stop-color="#E9EBEE"/>"##),
            "{svg}"
        );
        assert!(svg.contains(r#"cx="62" cy="46" r="70""#));
        assert!(svg.starts_with("<svg") && svg.ends_with("</svg>"));
    }

    #[test]
    fn wedges_fade_from_the_mount_to_the_window_edge() {
        use crate::presentation::stage_layout::{Rect, vehicle_layout};

        let colors = &Theme::embedded().colors;
        let layout = vehicle_layout(Rect::new(0.0, 0.0, 1000.0, 700.0), &Theme::embedded().stage);
        let svg = wedges_svg(
            1000.0,
            700.0,
            &layout.wedges,
            Some(CameraId::Front),
            (colors.wedge, colors.wedge_active),
            0.5,
        );
        assert_eq!(svg.matches("<polygon").count(), 6);
        // The active wedge is drive at 28 % × 0.5, the others ink at 7 % × 0.5.
        assert!(
            svg.contains(r##"<stop offset="0" stop-color="#3E6AE1" stop-opacity="0.1392"/>"##),
            "{svg}"
        );
        assert_eq!(
            svg.matches(r##"stop-color="#171A20" stop-opacity="0.0353""##)
                .count(),
            5
        );
        let front = layout.wedges[CameraId::Front];
        assert!(svg.contains(&format!(
            r#"x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}""#,
            front.mount.0, front.mount.1, front.edge_midpoint.0, front.edge_midpoint.1
        )));
        assert!(
            svg.starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="700""#)
        );
    }

    #[test]
    fn the_progress_ring_draws_the_done_part_from_twelve_o_clock() {
        let colors = &Theme::embedded().colors;
        let half = progress_ring_svg(0.5, colors.line, colors.drive);
        // r = 8: circumference 50.27, half of it left undrawn.
        assert!(half.contains(r#"stroke-dasharray="50.27""#), "{half}");
        assert!(half.contains(r#"stroke-dashoffset="25.13""#), "{half}");
        assert!(half.contains(r##"stroke="#E2E4E8""##) && half.contains(r##"stroke="#3E6AE1""##));
        assert!(half.contains(r#"transform="rotate(-90 10 10)""#));
        let clamped = progress_ring_svg(3.0, colors.line, colors.drive);
        assert!(clamped.contains(r#"stroke-dashoffset="0.00""#), "{clamped}");
    }

    #[test]
    fn the_car_shadow_is_ink_at_twenty_two_percent() {
        let svg = car_shadow_svg(Theme::embedded().colors.ink);
        assert!(
            svg.contains(r##"stop-color="#171A20" stop-opacity="0.22""##),
            "{svg}"
        );
        assert_eq!(CAR_SHADOW_BOX.2, 1.0 + 2.0 * 0.18);
    }
}
