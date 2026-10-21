//! The clip file-name grammar: `YYYY-MM-DD_hh-mm-ss-<camera>.mp4`.

use crate::camera::CameraId;
use crate::timestamp::parse_filename_local_timestamp;

/// A recognised clip file name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedClipName {
    /// The file-name timestamp, e.g. `2026-08-21_14-30-45`.
    pub timestamp: String,
    /// The camera the clip belongs to.
    pub camera: CameraId,
}

/// Parses a clip file name.
///
/// The match is case-sensitive and ASCII-only: `.MP4`, `Front` or full-width digits are rejected, and
/// the timestamp must be a valid calendar moment (`2026-02-29` is not). Helper files (`event.mp4`,
/// `thumb.png`, `event.json`) and unknown cameras such as `cabin` return `None`.
pub fn parse_clip_file_name(name: &str) -> Option<ParsedClipName> {
    let stem = name.strip_suffix(".mp4")?;
    let timestamp = stem.get(..19)?;
    let camera = stem.get(19..)?.strip_prefix('-')?;
    let camera_matches_grammar = !camera.is_empty()
        && camera
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'_');
    if !camera_matches_grammar {
        return None;
    }
    let camera = CameraId::from_id(camera)?;
    parse_filename_local_timestamp(timestamp)?;
    Some(ParsedClipName {
        timestamp: timestamp.to_owned(),
        camera,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;
    use crate::contract::run_fixture;

    #[test]
    fn matches_the_clip_filename_fixture() {
        run_fixture("clip-filename", |case| match case.function.as_str() {
            "parseClipFileName" => {
                let name = case.input("name").as_str().expect("name");
                Some(parse_clip_file_name(name).map_or(
                    Value::Null,
                    |parsed| json!({"timestamp": parsed.timestamp, "camera": parsed.camera.id()}),
                ))
            }
            _ => None,
        });
    }

    #[test]
    fn multibyte_names_do_not_panic() {
        assert_eq!(
            parse_clip_file_name("あいうえおかきくけこさしすせそたちつてと.mp4"),
            None
        );
        assert_eq!(parse_clip_file_name("é.mp4"), None);
    }
}
