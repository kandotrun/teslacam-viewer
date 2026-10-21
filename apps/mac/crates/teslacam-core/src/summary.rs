//! Event summaries: which cameras have footage, durations and counts.

use crate::camera::CameraId;
use crate::copy;
use crate::library::Event;
use crate::timeline;

/// Which cameras have footage in every segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraAvailability {
    /// Cameras present in every segment.
    pub available: usize,
    /// Always 6.
    pub total: usize,
    /// Cameras missing from at least one segment (all six when there is no segment), canonical order.
    pub missing: Vec<CameraId>,
}

/// Camera availability of `event`.
pub fn camera_availability(event: &Event) -> CameraAvailability {
    let missing = CameraId::ALL
        .into_iter()
        .filter(|camera| {
            event.segments.is_empty()
                || event
                    .segments
                    .iter()
                    .any(|segment| segment.cameras[*camera].is_none())
        })
        .collect::<Vec<_>>();
    CameraAvailability {
        available: CameraId::ALL.len() - missing.len(),
        total: CameraId::ALL.len(),
        missing,
    }
}

/// `{available}/{total} カメラ` (web layout).
pub fn camera_availability_label(availability: &CameraAvailability) -> String {
    copy::camera_availability_label(availability.available, availability.total)
}

/// `…なし` (web layout); `None` when nothing is missing.
pub fn missing_camera_label(availability: &CameraAvailability) -> Option<String> {
    copy::missing_camera_label(&availability.missing)
}

/// The nominal duration label, e.g. `11分` for 11 segments.
pub fn event_duration_label(event: &Event) -> String {
    copy::format_duration(timeline::event_duration(event.segments.len()) as u64)
}

/// `{n}セグメント`.
pub fn segment_count_label(event: &Event) -> String {
    copy::segment_count_label(event.segments.len())
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::contract::{event_input, option_json, run_fixture};

    #[test]
    fn matches_the_event_summary_fixture() {
        run_fixture("event-summary", |case| {
            let event = event_input(case.input("event"));
            let availability = camera_availability(&event);
            Some(match case.function.as_str() {
                "cameraAvailability" => json!({
                    "available": availability.available,
                    "total": availability.total,
                    "missing": availability.missing.iter().map(|camera| camera.id()).collect::<Vec<_>>(),
                }),
                "cameraAvailabilityLabel" => camera_availability_label(&availability).into(),
                "missingCameraLabel" => option_json(missing_camera_label(&availability)),
                "eventDuration" => Value::from(timeline::event_duration(event.segments.len())),
                "eventDurationLabel" => event_duration_label(&event).into(),
                "segmentCountLabel" => segment_count_label(&event).into(),
                _ => return None,
            })
        });
    }
}
