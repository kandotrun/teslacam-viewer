//! Scrubber geometry of the transport (`DESIGN.md` §6 Transport): where the playhead, the segment
//! ticks, the Sentry detection and the pre-roll start sit along the track, and which time a pointer
//! position means.
//!
//! Positions are fractions 0…1 of the rail. The rail is inset by half a thumb (10 pt) on both sides
//! of the track element, so the thumb's centre is on the rail at both ends.

use teslacam_core::detection::SentryDetection;

/// Half the thumb: the rail starts and ends this far inside the scrubber element.
pub const RAIL_INSET: f32 = 10.0;

/// `seconds` as a fraction of `duration` (clamped; 0 for an empty event or NaN).
pub fn fraction(seconds: f64, duration: f64) -> f32 {
    if duration.is_nan() || duration <= 0.0 || seconds.is_nan() {
        return 0.0;
    }
    (seconds / duration).clamp(0.0, 1.0) as f32
}

/// The time under a pointer at window x `x` over a scrubber element starting at `left` and
/// `width` wide.
pub fn seconds_at(x: f32, left: f32, width: f32, duration: f64) -> f64 {
    let rail = (width - 2.0 * RAIL_INSET).max(0.0);
    if rail <= 0.0 || duration.is_nan() || duration <= 0.0 {
        return 0.0;
    }
    let fraction = ((x - left - RAIL_INSET) / rail).clamp(0.0, 1.0);
    f64::from(fraction) * duration
}

/// Seconds a focused scrubber moves for `←` / `→` (the same step as the theater shortcuts).
pub const SLIDER_STEP_SECONDS: f64 = 10.0;

/// Where `key` moves a focused scrubber at `position` of `duration`: `←`/`→` by
/// [`SLIDER_STEP_SECONDS`], `Home`/`End` to the ends. `None` for other keys.
pub fn slider_key_target(key: &str, position: f64, duration: f64) -> Option<f64> {
    let duration = if duration.is_finite() {
        duration.max(0.0)
    } else {
        0.0
    };
    let target = match key {
        "left" => position - SLIDER_STEP_SECONDS,
        "right" => position + SLIDER_STEP_SECONDS,
        "home" => 0.0,
        "end" => duration,
        _ => return None,
    };
    Some(target.clamp(0.0, duration))
}

/// The marks drawn on the scrubber.
#[derive(Debug, Clone, PartialEq)]
pub struct ScrubberMarks {
    /// Segment boundaries after the first segment (2 × 12 pt ticks).
    pub ticks: Vec<f32>,
    /// The Sentry detection (`検知`).
    pub detection: Option<f32>,
    /// Where playback started when it started at the pre-roll (`検知の15秒前`).
    pub pre_roll: Option<f32>,
}

/// The marks of an event whose segments start at `segment_starts`. `detection` is the Sentry
/// detection (`None` for other categories); `representative` hides the detection marks, because
/// the six-camera clock is not what plays then.
pub fn scrubber_marks(
    segment_starts: &[f64],
    duration: f64,
    detection: Option<&SentryDetection>,
    representative: bool,
) -> ScrubberMarks {
    let detection = detection.filter(|_| !representative && duration > 0.0);
    ScrubberMarks {
        ticks: segment_starts
            .iter()
            .filter(|start| **start > 0.0 && **start < duration)
            .map(|start| fraction(*start, duration))
            .collect(),
        detection: detection.map(|detection| fraction(detection.seconds, duration)),
        pre_roll: detection
            .and_then(|detection| detection.pre_roll_start)
            .map(|start| fraction(start, duration)),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn slider_keys_step_by_ten_seconds_and_jump_to_the_ends() {
        use super::slider_key_target as key;
        assert_eq!(key("right", 30.0, 60.0), Some(40.0));
        assert_eq!(key("left", 30.0, 60.0), Some(20.0));
        assert_eq!(key("left", 4.0, 60.0), Some(0.0));
        assert_eq!(key("right", 55.0, 60.0), Some(60.0));
        assert_eq!(key("home", 30.0, 60.0), Some(0.0));
        assert_eq!(key("end", 30.0, 60.0), Some(60.0));
        assert_eq!(key("space", 30.0, 60.0), None);
        assert_eq!(key("end", 0.0, f64::NAN), Some(0.0));
    }

    use teslacam_core::timestamp::parse_metadata_local_timestamp;

    use super::*;

    fn detection(seconds: f64) -> SentryDetection {
        SentryDetection {
            moment: parse_metadata_local_timestamp("2026-08-20T20:35:30").expect("moment"),
            seconds,
            pre_roll_start: (seconds >= 15.0).then_some(seconds - 15.0),
        }
    }

    #[test]
    fn fractions_are_clamped() {
        assert_eq!(fraction(15.0, 60.0), 0.25);
        assert_eq!(fraction(-3.0, 60.0), 0.0);
        assert_eq!(fraction(90.0, 60.0), 1.0);
        assert_eq!(fraction(10.0, 0.0), 0.0);
        assert_eq!(fraction(f64::NAN, 60.0), 0.0);
    }

    #[test]
    fn the_pointer_maps_onto_the_inset_rail() {
        // A 220 pt scrubber at x = 100: the rail runs from 110 to 310.
        assert_eq!(seconds_at(110.0, 100.0, 220.0, 120.0), 0.0);
        assert_eq!(seconds_at(210.0, 100.0, 220.0, 120.0), 60.0);
        assert_eq!(seconds_at(310.0, 100.0, 220.0, 120.0), 120.0);
        assert_eq!(seconds_at(50.0, 100.0, 220.0, 120.0), 0.0);
        assert_eq!(seconds_at(500.0, 100.0, 220.0, 120.0), 120.0);
        assert_eq!(seconds_at(200.0, 100.0, 10.0, 120.0), 0.0);
    }

    #[test]
    fn marks_show_segment_boundaries_and_the_sentry_detection() {
        let marks = scrubber_marks(&[0.0, 60.0, 120.0], 180.0, Some(&detection(90.0)), false);
        assert_eq!(marks.ticks, vec![1.0 / 3.0, 2.0 / 3.0]);
        assert_eq!(marks.detection, Some(0.5));
        assert_eq!(marks.pre_roll, Some(75.0 / 180.0));
    }

    #[test]
    fn early_detections_have_no_pre_roll_and_the_representative_view_hides_both() {
        let early = scrubber_marks(&[0.0], 60.0, Some(&detection(10.0)), false);
        assert_eq!(early.detection, Some(10.0 / 60.0));
        assert_eq!(early.pre_roll, None);
        let representative = scrubber_marks(&[0.0, 60.0], 120.0, Some(&detection(90.0)), true);
        assert_eq!(representative.detection, None);
        assert_eq!(representative.pre_roll, None);
        assert_eq!(representative.ticks, vec![0.5]);
        let other = scrubber_marks(&[0.0], 60.0, None, false);
        assert_eq!((other.detection, other.pre_roll), (None, None));
        assert!(other.ticks.is_empty());
    }
}
