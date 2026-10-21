//! `event.json` metadata of Saved and Sentry events.

use std::io;

use serde_json::Value;

use crate::limits::{
    MAX_METADATA_STRING_LENGTH, ScanLimitError, ScanLimitKind, ensure_metadata_size,
};

/// The known `event.json` fields. Unknown fields are ignored.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EventMetadata {
    /// Detection time, e.g. `2026-08-20T20:35:30` (format checked later, see [`crate::detection`]).
    pub timestamp: String,
    /// City (Tesla's estimate).
    pub city: Option<String>,
    /// Street (Tesla's estimate).
    pub street: Option<String>,
    /// Estimated latitude, as written by the car.
    pub est_lat: Option<String>,
    /// Estimated longitude, as written by the car.
    pub est_lon: Option<String>,
    /// Why the clip was recorded, e.g. `sentry_aware_object_detection`.
    pub reason: Option<String>,
    /// The triggering camera; stored but never displayed.
    pub camera: Option<String>,
}

const KNOWN_FIELDS: [&str; 7] = [
    "timestamp",
    "city",
    "street",
    "est_lat",
    "est_lon",
    "reason",
    "camera",
];

fn utf16_length(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Validates a parsed `event.json` value.
///
/// Returns `Ok(None)` when the value is not an object, `timestamp` is missing or not a string, or an
/// optional known field is present but not a string (JSON `null` included). Any known string field
/// longer than 2048 UTF-16 code units fails the scan, even when other fields are invalid.
pub fn parse_event_metadata(value: &Value) -> Result<Option<EventMetadata>, ScanLimitError> {
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    let too_long = KNOWN_FIELDS.iter().any(|field| {
        object
            .get(*field)
            .and_then(Value::as_str)
            .is_some_and(|text| utf16_length(text) > MAX_METADATA_STRING_LENGTH)
    });
    if too_long {
        return Err(ScanLimitError::new(ScanLimitKind::MetadataString));
    }
    let Some(timestamp) = object.get("timestamp").and_then(Value::as_str) else {
        return Ok(None);
    };
    let mut invalid = false;
    let mut optional = |field: &str| match object.get(field) {
        None => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(_) => {
            invalid = true;
            None
        }
    };
    let metadata = EventMetadata {
        timestamp: timestamp.to_owned(),
        city: optional("city"),
        street: optional("street"),
        est_lat: optional("est_lat"),
        est_lon: optional("est_lon"),
        reason: optional("reason"),
        camera: optional("camera"),
    };
    Ok((!invalid).then_some(metadata))
}

/// Decodes and validates the bytes of an `event.json` whose size was already checked.
///
/// Decoding follows `Blob.text()`: one leading UTF-8 byte order mark is stripped and invalid
/// sequences become U+FFFD. Invalid JSON returns `Ok(None)`; duplicate keys keep the last value.
pub fn decode_event_metadata(bytes: &[u8]) -> Result<Option<EventMetadata>, ScanLimitError> {
    let text = String::from_utf8_lossy(bytes);
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
    match serde_json::from_str::<Value>(text) {
        Ok(value) => parse_event_metadata(&value),
        Err(_) => Ok(None),
    }
}

/// Reads an `event.json`: checks the size recorded during the scan, reads at most one byte more than
/// the limit, checks the actual size and decodes.
///
/// `read` receives the maximum number of bytes to read. An I/O error makes the file unreadable
/// (`Ok(None)`), never fails the scan.
pub fn read_event_metadata(
    declared_size: u64,
    read: impl FnOnce(u64) -> io::Result<Vec<u8>>,
) -> Result<Option<EventMetadata>, ScanLimitError> {
    ensure_metadata_size(declared_size)?;
    let bytes = match read(crate::limits::MAX_EVENT_METADATA_SIZE + 1) {
        Ok(bytes) => bytes,
        Err(_) => return Ok(None),
    };
    ensure_metadata_size(bytes.len() as u64)?;
    decode_event_metadata(&bytes)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::contract::{metadata_json, run_fixture, string_input};

    fn expand(value: &Value) -> Value {
        match value {
            Value::Object(object) if object.contains_key("$repeat") => {
                Value::String(string_input(value))
            }
            Value::Object(object) => Value::Object(
                object
                    .iter()
                    .map(|(key, item)| (key.clone(), expand(item)))
                    .collect(),
            ),
            Value::Array(items) => Value::Array(items.iter().map(expand).collect()),
            other => other.clone(),
        }
    }

    fn result_json(result: Result<Option<EventMetadata>, ScanLimitError>) -> Value {
        match result {
            Ok(Some(metadata)) => json!({"result": "ok", "metadata": metadata_json(&metadata)}),
            Ok(None) => json!({"result": "invalid"}),
            Err(error) => json!({"result": "scan-limit", "kind": error.kind.id()}),
        }
    }

    fn decode_base64(text: &str) -> Vec<u8> {
        const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut bits = 0u32;
        let mut count = 0;
        let mut bytes = Vec::new();
        for byte in text.bytes().filter(|byte| *byte != b'=') {
            let value = ALPHABET
                .iter()
                .position(|candidate| *candidate == byte)
                .expect("base64") as u32;
            bits = bits << 6 | value;
            count += 6;
            if count >= 8 {
                count -= 8;
                bytes.push((bits >> count) as u8);
                bits &= (1 << count) - 1;
            }
        }
        bytes
    }

    #[test]
    fn matches_the_event_metadata_fixture() {
        run_fixture("event-metadata", |case| {
            Some(match case.function.as_str() {
                "parseEventMetadata" => {
                    result_json(parse_event_metadata(&expand(case.input("json"))))
                }
                "readEventMetadata" => {
                    let bytes = match case.input.get("text") {
                        Some(text) => string_input(text).into_bytes(),
                        None => decode_base64(case.input("base64").as_str().expect("base64")),
                    };
                    let declared = case
                        .input
                        .get("declaredSize")
                        .and_then(Value::as_u64)
                        .unwrap_or(bytes.len() as u64);
                    result_json(read_event_metadata(declared, |limit| {
                        Ok(bytes.iter().copied().take(limit as usize).collect())
                    }))
                }
                _ => return None,
            })
        });
    }

    #[test]
    fn io_errors_make_the_file_unreadable() {
        let result = read_event_metadata(10, |_| Err(io::Error::other("removed SSD")));
        assert_eq!(result, Ok(None));
    }

    #[test]
    fn oversized_declared_files_are_never_read() {
        let result = read_event_metadata(crate::limits::MAX_EVENT_METADATA_SIZE + 1, |_| {
            panic!("must not read")
        });
        assert_eq!(
            result.map_err(|error| error.kind),
            Err(ScanLimitKind::MetadataSize)
        );
    }
}
