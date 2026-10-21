//! Motion tokens (`DESIGN.md` §7) as functions of elapsed time.
//!
//! - [`CubicBezier`] evaluates CSS `cubic-bezier(x1, y1, x2, y2)` easings.
//! - [`Reveal`] is the one orchestrated moment: after a folder loads, the six windows fill with
//!   video in mount order, `reveal-stagger-ms` apart, each over `motion-sheet`. "Reduce motion" sets
//!   every duration to 0.

use std::time::Duration;

use teslacam_core::camera::CameraId;
use teslacam_core::theme::Motion;

/// A CSS cubic Bézier easing from (0, 0) to (1, 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubicBezier {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
}

impl CubicBezier {
    /// The easing `cubic-bezier(x1, y1, x2, y2)`.
    pub const fn new([x1, y1, x2, y2]: [f32; 4]) -> CubicBezier {
        CubicBezier { x1, y1, x2, y2 }
    }

    fn sample(a1: f32, a2: f32, t: f32) -> f32 {
        // B(t) = 3(1−t)²t·a1 + 3(1−t)t²·a2 + t³
        let u = 1.0 - t;
        3.0 * u * u * t * a1 + 3.0 * u * t * t * a2 + t * t * t
    }

    /// Progress (0…1) at time fraction `x` (clamped to 0…1).
    pub fn ease(&self, x: f32) -> f32 {
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }
        // Bisection on the monotonic x(t); 24 steps are far below a pixel.
        let (mut low, mut high) = (0.0f32, 1.0f32);
        for _ in 0..24 {
            let middle = (low + high) / 2.0;
            if Self::sample(self.x1, self.x2, middle) < x {
                low = middle;
            } else {
                high = middle;
            }
        }
        Self::sample(self.y1, self.y2, (low + high) / 2.0)
    }
}

/// The staggered fill of the six windows after a folder loads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reveal {
    duration: Duration,
    stagger: Duration,
    easing: CubicBezier,
}

impl Reveal {
    /// The reveal of `motion` (`motion-sheet` per window, `reveal-stagger-ms` apart); instant when
    /// `reduce_motion` is set.
    pub fn new(motion: &Motion, reduce_motion: bool) -> Reveal {
        let millis =
            |value: u32| Duration::from_millis(if reduce_motion { 0 } else { u64::from(value) });
        Reveal {
            duration: millis(motion.sheet.duration_ms),
            stagger: millis(motion.reveal_stagger_ms),
            easing: CubicBezier::new(motion.sheet.easing),
        }
    }

    fn delay(&self, camera: CameraId) -> Duration {
        let order = CameraId::REVEAL_ORDER
            .iter()
            .position(|candidate| *candidate == camera)
            .unwrap_or(0);
        self.stagger * order as u32
    }

    /// How far `camera`'s window has filled `elapsed` after the folder loaded (0 = empty, 1 = video).
    pub fn progress(&self, camera: CameraId, elapsed: Duration) -> f32 {
        let Some(running) = elapsed.checked_sub(self.delay(camera)) else {
            return 0.0;
        };
        if self.duration.is_zero() {
            return 1.0;
        }
        self.easing
            .ease(running.as_secs_f32() / self.duration.as_secs_f32())
    }

    /// When every window has filled.
    pub fn total(&self) -> Duration {
        self.delay(CameraId::REVEAL_ORDER[5]) + self.duration
    }
}

#[cfg(test)]
mod tests {
    use teslacam_core::theme::Theme;

    use super::*;

    fn close(left: f32, right: f32) -> bool {
        (left - right).abs() < 1e-3
    }

    #[test]
    fn linear_and_standard_easings() {
        let linear = CubicBezier::new([0.0, 0.0, 1.0, 1.0]);
        for x in [0.0, 0.25, 0.5, 0.9, 1.0] {
            assert!(close(linear.ease(x), x));
        }
        // CSS `ease` = cubic-bezier(.25, .1, .25, 1) is 0.8024 halfway.
        let ease = CubicBezier::new([0.25, 0.1, 0.25, 1.0]);
        assert!(close(ease.ease(0.5), 0.8024), "{}", ease.ease(0.5));
        let sheet = CubicBezier::new(Theme::embedded().motion.sheet.easing);
        assert!(close(sheet.ease(0.0), 0.0) && close(sheet.ease(1.0), 1.0));
        assert!(sheet.ease(0.5) > 0.8, "a decelerating curve");
        assert!(close(sheet.ease(-1.0), 0.0) && close(sheet.ease(2.0), 1.0));
    }

    #[test]
    fn windows_fill_in_mount_order_forty_milliseconds_apart() {
        let reveal = Reveal::new(&Theme::embedded().motion, false);
        let at = |millis| Duration::from_millis(millis);
        assert_eq!(reveal.progress(CameraId::Front, at(0)), 0.0);
        assert!(reveal.progress(CameraId::Front, at(100)) > 0.0);
        assert_eq!(reveal.progress(CameraId::LeftPillar, at(39)), 0.0);
        assert!(reveal.progress(CameraId::LeftPillar, at(60)) > 0.0);
        // The back window is the last one.
        assert_eq!(reveal.progress(CameraId::Back, at(199)), 0.0);
        assert!(close(reveal.progress(CameraId::Back, at(200 + 320)), 1.0));
        assert_eq!(reveal.total(), at(520));
        assert!(close(reveal.progress(CameraId::Front, at(10_000)), 1.0));
    }

    #[test]
    fn reduce_motion_fills_every_window_at_once() {
        let reveal = Reveal::new(&Theme::embedded().motion, true);
        for camera in CameraId::ALL {
            assert_eq!(reveal.progress(camera, Duration::ZERO), 1.0);
        }
        assert_eq!(reveal.total(), Duration::ZERO);
    }
}
