//! The web's nominal playback axis: every segment occupies a 60 s window regardless of real clip
//! lengths or gaps. The Mac plays on the wall-clock axis of [`crate::event_timeline`], which agrees
//! with this one for a regular 60 s cadence; this module keeps the web rule for parity checks and for
//! the web-layout labels of [`crate::summary`].

/// Nominal seconds per segment.
pub const SEGMENT_SECONDS: f64 = 60.0;

/// A position inside an event.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaybackLocation {
    /// Index of the segment.
    pub segment_index: usize,
    /// Seconds into that segment.
    pub offset_seconds: f64,
}

/// `segment_count × 60` seconds.
pub fn event_duration(segment_count: usize) -> f64 {
    segment_count as f64 * SEGMENT_SECONDS
}

/// Locates `elapsed_seconds` on the nominal axis.
///
/// NaN becomes 0 and the value is clamped to the event. Boundaries belong to the next segment
/// (60 → segment 1, offset 0) except the end, which stays in the last segment (3 segments, 180 →
/// segment 2, offset 60). An event without segments locates everything at segment 0, offset 0.
pub fn locate_playback_time(segment_count: usize, elapsed_seconds: f64) -> PlaybackLocation {
    let duration = event_duration(segment_count);
    if segment_count == 0 {
        return PlaybackLocation {
            segment_index: 0,
            offset_seconds: 0.0,
        };
    }
    let normalized = if elapsed_seconds.is_nan() {
        0.0
    } else {
        elapsed_seconds
    };
    let clamped = normalized.clamp(0.0, duration);
    let segment_index = ((clamped / SEGMENT_SECONDS).floor() as usize).min(segment_count - 1);
    PlaybackLocation {
        segment_index,
        offset_seconds: clamped - segment_index as f64 * SEGMENT_SECONDS,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::contract::{number_input, run_fixture};

    fn segment_count(case: &crate::contract::Case) -> usize {
        case.input("segmentCount").as_u64().expect("segmentCount") as usize
    }

    #[test]
    fn matches_the_playback_timeline_fixture() {
        run_fixture("playback-timeline", |case| {
            Some(match case.function.as_str() {
                "eventDuration" => Value::from(event_duration(segment_count(case))),
                "locatePlaybackTime" => {
                    let location = locate_playback_time(
                        segment_count(case),
                        number_input(case.input("elapsedSeconds")),
                    );
                    json!({"segmentIndex": location.segment_index, "offsetSeconds": location.offset_seconds})
                }
                _ => return None,
            })
        });
    }
}
