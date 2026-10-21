//! Test support: loads the golden fixtures in `packages/contract/fixtures` and compares results.
//!
//! Every module's tests call [`run_fixture`] with a dispatcher that maps a case's `fn` and `input` to
//! the Rust result as JSON. A case whose `fn` the dispatcher does not know fails the test, and
//! [`tests::every_fixture_file_has_a_consumer`] fails when a fixture file is not listed in
//! [`FIXTURE_FILES`], so nothing is skipped silently.

use std::path::PathBuf;

use serde_json::{Map, Value};

use crate::camera::{CameraId, CameraMap, ClipCategory};
use crate::library::{Event, FileId, Segment};
use crate::metadata::EventMetadata;

/// Every fixture file and the module whose tests consume it.
pub(crate) const FIXTURE_FILES: &[(&str, &str)] = &[
    ("clip-filename", "filename"),
    ("copy-format", "copy"),
    ("day-timeline", "day_timeline"),
    ("directory-walk", "scan"),
    ("event-detection", "detection"),
    ("event-format-filters", "filters"),
    ("event-metadata", "metadata"),
    ("event-summary", "summary"),
    ("library-build", "library"),
    ("local-timestamp", "timestamp"),
    ("playback-timeline", "timeline / event_timeline"),
];

const DEFAULT_FLOAT_TOLERANCE: f64 = 1e-9;

pub(crate) fn fixtures_directory() -> PathBuf {
    PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../packages/contract/fixtures"
    ))
}

/// One fixture case.
pub(crate) struct Case {
    pub name: String,
    pub function: String,
    pub input: Value,
    pub expected: Value,
}

impl Case {
    pub fn input(&self, key: &str) -> &Value {
        self.input
            .get(key)
            .unwrap_or_else(|| panic!("case {:?}: input has no {key:?}", self.name))
    }
}

/// Loads every case of `file` that applies to the Mac.
pub(crate) fn load_cases(file: &str) -> (Vec<Case>, f64) {
    assert!(
        FIXTURE_FILES.iter().any(|(name, _)| *name == file),
        "{file} is not listed in FIXTURE_FILES"
    );
    let path = fixtures_directory().join(format!("{file}.json"));
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let root: Value = serde_json::from_str(&text).expect("fixture is valid JSON");
    assert_eq!(
        root["contract"], "teslacam-viewer",
        "{file}: wrong contract"
    );
    assert_eq!(root["schemaVersion"], 1, "{file}: unknown schema version");
    let tolerance = root["floatTolerance"]
        .as_f64()
        .unwrap_or(DEFAULT_FLOAT_TOLERANCE);
    let cases = root["cases"]
        .as_array()
        .expect("cases array")
        .iter()
        .filter(|case| applies_to_mac(case))
        .map(|case| Case {
            name: case["name"].as_str().expect("case name").to_owned(),
            function: case["fn"].as_str().expect("case fn").to_owned(),
            input: case["input"].clone(),
            expected: case["expected"].clone(),
        })
        .collect::<Vec<_>>();
    assert!(!cases.is_empty(), "{file}: no cases for the Mac");
    (cases, tolerance)
}

fn applies_to_mac(case: &Value) -> bool {
    match case.get("platforms") {
        None => true,
        Some(platforms) => platforms
            .as_array()
            .expect("platforms array")
            .iter()
            .any(|platform| platform == "mac"),
    }
}

/// Runs every case of `file` through `dispatch` and panics with all mismatches.
///
/// `dispatch` returns `None` for an unknown `fn`, which is reported as a failure.
pub(crate) fn run_fixture(file: &str, dispatch: impl Fn(&Case) -> Option<Value>) {
    let (cases, tolerance) = load_cases(file);
    let mut failures = Vec::new();
    for case in &cases {
        match dispatch(case) {
            None => failures.push(format!("{}: unhandled fn {:?}", case.name, case.function)),
            Some(actual) => {
                if let Err(path) = json_close(&actual, &case.expected, tolerance, "$") {
                    failures.push(format!(
                        "{} ({}): mismatch at {path}\n  expected {}\n  actual   {}",
                        case.name,
                        case.function,
                        truncate(&case.expected),
                        truncate(&actual)
                    ));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{file}.json: {} of {} cases failed\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

fn truncate(value: &Value) -> String {
    let text = value.to_string();
    if text.chars().count() > 400 {
        format!("{}…", text.chars().take(400).collect::<String>())
    } else {
        text
    }
}

/// Compares JSON values: objects by key set, arrays by order, numbers with an absolute tolerance.
pub(crate) fn json_close(
    actual: &Value,
    expected: &Value,
    tolerance: f64,
    path: &str,
) -> Result<(), String> {
    match (actual, expected) {
        (Value::Number(left), Value::Number(right)) => {
            let (left, right) = (
                left.as_f64().unwrap_or(f64::NAN),
                right.as_f64().unwrap_or(f64::NAN),
            );
            if (left - right).abs() <= tolerance {
                Ok(())
            } else {
                Err(path.to_owned())
            }
        }
        (Value::Array(left), Value::Array(right)) => {
            if left.len() != right.len() {
                return Err(format!("{path} (length {} vs {})", left.len(), right.len()));
            }
            for (index, (left, right)) in left.iter().zip(right).enumerate() {
                json_close(left, right, tolerance, &format!("{path}[{index}]"))?;
            }
            Ok(())
        }
        (Value::Object(left), Value::Object(right)) => {
            let mut keys = left.keys().chain(right.keys()).collect::<Vec<_>>();
            keys.sort();
            keys.dedup();
            for key in keys {
                match (left.get(key), right.get(key)) {
                    (Some(left), Some(right)) => {
                        json_close(left, right, tolerance, &format!("{path}.{key}"))?
                    }
                    _ => return Err(format!("{path}.{key} (missing on one side)")),
                }
            }
            Ok(())
        }
        _ if actual == expected => Ok(()),
        _ => Err(path.to_owned()),
    }
}

// ---------------------------------------------------------------------------------------------
// Input decoding shared by the module tests.
// ---------------------------------------------------------------------------------------------

/// A string input, expanding `{"$repeat": s, "count": n}`.
pub(crate) fn string_input(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Object(object) => {
            let repeat = object["$repeat"].as_str().expect("$repeat string");
            let count = object["count"].as_u64().expect("count") as usize;
            repeat.repeat(count)
        }
        other => panic!("not a string input: {other}"),
    }
}

/// A number input, decoding the strings `NaN`, `Infinity` and `-Infinity`.
pub(crate) fn number_input(value: &Value) -> f64 {
    match value {
        Value::Number(number) => number.as_f64().expect("finite number"),
        Value::String(text) => match text.as_str() {
            "NaN" => f64::NAN,
            "Infinity" => f64::INFINITY,
            "-Infinity" => f64::NEG_INFINITY,
            other => panic!("not a number input: {other}"),
        },
        other => panic!("not a number input: {other}"),
    }
}

pub(crate) fn optional_string(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

pub(crate) fn camera_input(value: &Value) -> CameraId {
    CameraId::from_id(value.as_str().expect("camera id")).expect("known camera id")
}

pub(crate) fn category_input(value: &Value) -> ClipCategory {
    ClipCategory::from_id(value.as_str().expect("category id")).expect("known category")
}

pub(crate) fn metadata_input(value: &Value) -> EventMetadata {
    let field = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
    EventMetadata {
        timestamp: field("timestamp").expect("metadata timestamp"),
        city: field("city"),
        street: field("street"),
        est_lat: field("est_lat"),
        est_lon: field("est_lon"),
        reason: field("reason"),
        camera: field("camera"),
    }
}

pub(crate) fn metadata_json(metadata: &EventMetadata) -> Value {
    let mut object = Map::new();
    object.insert("timestamp".into(), Value::from(metadata.timestamp.clone()));
    for (key, value) in [
        ("city", &metadata.city),
        ("street", &metadata.street),
        ("est_lat", &metadata.est_lat),
        ("est_lon", &metadata.est_lon),
        ("reason", &metadata.reason),
        ("camera", &metadata.camera),
    ] {
        if let Some(value) = value {
            object.insert(key.into(), Value::from(value.clone()));
        }
    }
    Value::Object(object)
}

/// Builds an [`Event`] from the compact `EventFixture` input. Files get placeholder ids.
pub(crate) fn event_input(value: &Value) -> Event {
    let mut next_file = 0usize;
    let mut placeholder = || {
        next_file += 1;
        FileId(next_file - 1)
    };
    let segments = value["segments"]
        .as_array()
        .expect("segments")
        .iter()
        .map(|segment| {
            let mut cameras = CameraMap::default();
            for camera in segment["cameras"].as_array().expect("cameras") {
                cameras[camera_input(camera)] = Some(placeholder());
            }
            Segment {
                timestamp: segment["timestamp"].as_str().expect("timestamp").to_owned(),
                cameras,
            }
        })
        .collect();
    let flag = |key: &str| value.get(key).and_then(Value::as_bool).unwrap_or(false);
    Event {
        id: value["id"].as_str().expect("event id").to_owned(),
        category: category_input(&value["category"]),
        timestamp: value["timestamp"]
            .as_str()
            .expect("event timestamp")
            .to_owned(),
        segments,
        representative: flag("hasRepresentative").then(&mut placeholder),
        thumbnail: flag("hasThumbnail").then(&mut placeholder),
        metadata: value.get("metadata").map(metadata_input),
    }
}

pub(crate) fn events_input(value: &Value) -> Vec<Event> {
    value
        .as_array()
        .expect("events")
        .iter()
        .map(event_input)
        .collect()
}

pub(crate) fn option_json<T: Into<Value>>(value: Option<T>) -> Value {
    value.map_or(Value::Null, Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_fixture_file_has_a_consumer() {
        let mut files = std::fs::read_dir(fixtures_directory())
            .expect("fixtures directory")
            .map(|entry| {
                entry
                    .expect("entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .filter(|name| name.ends_with(".json"))
            .map(|name| name.trim_end_matches(".json").to_owned())
            .collect::<Vec<_>>();
        files.sort();
        let listed = FIXTURE_FILES
            .iter()
            .map(|(name, _)| (*name).to_owned())
            .collect::<Vec<_>>();
        assert_eq!(files, listed);
    }

    #[test]
    fn numbers_compare_with_the_tolerance() {
        let tolerance = DEFAULT_FLOAT_TOLERANCE;
        assert!(json_close(&Value::from(1.0), &Value::from(1.0 + 1e-12), tolerance, "$").is_ok());
        assert!(json_close(&Value::from(1.0), &Value::from(1.001), tolerance, "$").is_err());
        assert!(json_close(&Value::from(3), &Value::from(3.0), tolerance, "$").is_ok());
    }

    #[test]
    fn objects_compare_by_key_set() {
        let left = serde_json::json!({"a": 1, "b": null});
        let right = serde_json::json!({"a": 1});
        assert!(json_close(&left, &right, 0.0, "$").is_err());
        assert!(json_close(&left, &left.clone(), 0.0, "$").is_ok());
    }

    #[test]
    fn generated_strings_expand() {
        let value = serde_json::json!({"$repeat": "ab", "count": 3});
        assert_eq!(string_input(&value), "ababab");
        assert_eq!(string_input(&Value::from("x")), "x");
    }

    #[test]
    fn non_finite_numbers_decode() {
        assert!(number_input(&Value::from("NaN")).is_nan());
        assert_eq!(number_input(&Value::from("Infinity")), f64::INFINITY);
        assert_eq!(number_input(&Value::from("-Infinity")), f64::NEG_INFINITY);
        assert_eq!(number_input(&Value::from(2.5)), 2.5);
    }
}
