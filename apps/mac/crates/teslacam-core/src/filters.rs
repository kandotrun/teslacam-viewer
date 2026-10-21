//! Event text and library filters (category, date, free-text query).

use crate::camera::ClipCategory;
use crate::copy::category_label;
use crate::library::Event;

/// The category filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CategoryFilter {
    /// Every category.
    #[default]
    All,
    /// One category.
    Only(ClipCategory),
}

impl CategoryFilter {
    /// Whether `category` passes.
    pub fn accepts(self, category: ClipCategory) -> bool {
        match self {
            CategoryFilter::All => true,
            CategoryFilter::Only(only) => only == category,
        }
    }
}

/// The library filters.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EventFilters {
    /// Category filter.
    pub category: CategoryFilter,
    /// `YYYY-MM-DD`; matched as a raw prefix of the event timestamp.
    pub date_key: Option<String>,
    /// Free text; trimmed and lower-cased before matching.
    pub query: String,
}

/// The date part of a timestamp: its first 10 characters.
pub fn event_date(timestamp: &str) -> String {
    timestamp.chars().take(10).collect()
}

/// The clock part of a timestamp: characters from index 11 with `-` replaced by `:`.
pub fn event_clock(timestamp: &str) -> String {
    timestamp
        .chars()
        .skip(11)
        .map(|character| if character == '-' { ':' } else { character })
        .collect()
}

/// `{date} {clock}`, e.g. `2026-08-20 20:45:00`.
pub fn event_time(timestamp: &str) -> String {
    format!("{} {}", event_date(timestamp), event_clock(timestamp))
}

/// White space removed by JavaScript's `String.prototype.trim` (WhiteSpace and LineTerminator).
///
/// Unlike Rust's `char::is_whitespace` it includes U+FEFF and excludes U+0085.
fn is_js_trim_whitespace(character: char) -> bool {
    matches!(
        character,
        '\t' | '\n' | '\u{000B}' | '\u{000C}' | '\r' | ' ' | '\u{00A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

/// The normalized query: trimmed like JavaScript's `trim` and lower-cased.
pub fn normalized_query(query: &str) -> String {
    query.trim_matches(is_js_trim_whitespace).to_lowercase()
}

/// Whether `event` passes every filter.
///
/// The query matches against `"{eventTime} {category id} {category label}"`, e.g.
/// `2026-08-20 20:45:00 sentry セントリー`.
pub fn event_matches_filters(event: &Event, filters: &EventFilters) -> bool {
    if !filters.category.accepts(event.category) {
        return false;
    }
    if let Some(date_key) = &filters.date_key
        && !event.timestamp.starts_with(date_key.as_str())
    {
        return false;
    }
    let query = normalized_query(&filters.query);
    query.is_empty()
        || format!(
            "{} {} {}",
            event_time(&event.timestamp),
            event.category.id(),
            category_label(event.category)
        )
        .to_lowercase()
        .contains(&query)
}

/// The matching events, newest first; ties keep their input order.
pub fn ordered_matching_events<'a>(
    events: impl IntoIterator<Item = &'a Event>,
    filters: &EventFilters,
) -> Vec<&'a Event> {
    let mut matching = events
        .into_iter()
        .filter(|event| event_matches_filters(event, filters))
        .collect::<Vec<_>>();
    matching.sort_by(|left, right| right.timestamp.cmp(&left.timestamp));
    matching
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::contract::{
        category_input, event_input, events_input, optional_string, run_fixture,
    };

    fn filters_input(value: &Value) -> EventFilters {
        EventFilters {
            category: match value["category"].as_str().expect("category") {
                "all" => CategoryFilter::All,
                _ => CategoryFilter::Only(category_input(&value["category"])),
            },
            date_key: optional_string(&value["dateKey"]),
            query: value["query"].as_str().expect("query").to_owned(),
        }
    }

    #[test]
    fn matches_the_event_format_filters_fixture() {
        run_fixture("event-format-filters", |case| {
            let timestamp = || case.input("timestamp").as_str().expect("timestamp");
            Some(match case.function.as_str() {
                "eventDate" => event_date(timestamp()).into(),
                "eventClock" => event_clock(timestamp()).into(),
                "eventTime" => event_time(timestamp()).into(),
                "categoryLabel" => category_label(category_input(case.input("category"))).into(),
                "eventMatchesFilters" => event_matches_filters(
                    &event_input(case.input("event")),
                    &filters_input(case.input("filters")),
                )
                .into(),
                "orderedMatchingEvents" => {
                    let events = events_input(case.input("events"));
                    ordered_matching_events(&events, &filters_input(case.input("filters")))
                        .into_iter()
                        .map(|event| Value::from(event.id.clone()))
                        .collect::<Vec<_>>()
                        .into()
                }
                _ => return None,
            })
        });
    }

    #[test]
    fn js_trim_differs_from_rust_trim() {
        assert_eq!(normalized_query("\u{FEFF} SENTRY \u{3000}"), "sentry");
        assert_eq!(normalized_query("\u{0085}x"), "\u{0085}x");
    }
}
