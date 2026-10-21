//! Which items the event pill shows at a given width (`DESIGN.md` §6 Event pill: "Items drop from
//! the right as the window narrows: detection, then place"). An item is shown whole or not at all.

/// Space between the pill's items.
pub const ITEM_GAP: f32 = 14.0;
/// Width of the separator before an optional item.
pub const SEPARATOR_WIDTH: f32 = 1.0;

/// Natural widths of the pill's parts.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PillWidths {
    /// Padding, category chip, start time and the gap between them: always shown.
    pub fixed: f32,
    /// The place item (pin icon + text), when the event has one.
    pub place: Option<f32>,
    /// The Sentry detection item, when the event has one.
    pub detection: Option<f32>,
}

/// The optional items that fit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PillItems {
    /// Show the place.
    pub place: bool,
    /// Show the detection.
    pub detection: bool,
}

/// What an optional item adds: a separator with a gap on each side, and the item.
fn cost(width: f32) -> f32 {
    2.0 * ITEM_GAP + SEPARATOR_WIDTH + width
}

/// The items that fit into `available` points: detection drops first, then place.
pub fn pill_items(available: f32, widths: PillWidths) -> PillItems {
    let place = widths.place.map_or(0.0, cost);
    let detection = widths.detection.map_or(0.0, cost);
    let fits = |total: f32| total <= available + 0.5;
    if fits(widths.fixed + place + detection) {
        PillItems {
            place: widths.place.is_some(),
            detection: widths.detection.is_some(),
        }
    } else {
        PillItems {
            place: widths.place.is_some() && fits(widths.fixed + place),
            detection: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WIDTHS: PillWidths = PillWidths {
        fixed: 200.0,
        place: Some(120.0),
        detection: Some(180.0),
    };

    #[test]
    fn everything_shows_when_it_fits() {
        let all = 200.0 + 3.0 * 29.0 / 3.0 * 2.0 + 120.0 + 180.0;
        assert_eq!(
            pill_items(all + 10.0, WIDTHS),
            PillItems {
                place: true,
                detection: true
            }
        );
    }

    #[test]
    fn detection_drops_first_then_place_whole() {
        // Room for the place but not for the detection as well.
        assert_eq!(
            pill_items(200.0 + 29.0 + 120.0 + 20.0, WIDTHS),
            PillItems {
                place: true,
                detection: false
            }
        );
        // Not even room for the whole place: it is dropped, never truncated.
        assert_eq!(
            pill_items(200.0 + 29.0 + 60.0, WIDTHS),
            PillItems {
                place: false,
                detection: false
            }
        );
    }

    #[test]
    fn missing_items_are_never_shown() {
        let none = PillWidths {
            fixed: 200.0,
            place: None,
            detection: None,
        };
        assert_eq!(
            pill_items(1000.0, none),
            PillItems {
                place: false,
                detection: false
            }
        );
    }
}
