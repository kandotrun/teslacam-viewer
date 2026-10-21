//! Safe local scan limits.
//!
//! Every limit error aborts the whole scan and is shown as the "failed" error state; there is no
//! dedicated copy for it.

use std::fmt;

/// Maximum entries enumerated during the walk, and maximum files handed to the library builder.
pub const MAX_SCAN_ENTRIES: u64 = 100_000;
/// Maximum directory depth below the selected folder (the folder itself is depth 0).
pub const MAX_DIRECTORY_DEPTH: usize = 8;
/// Maximum size of one `event.json`, declared and actually read.
pub const MAX_EVENT_METADATA_SIZE: u64 = 1024 * 1024;
/// Maximum length of each known `event.json` string field, in UTF-16 code units.
pub const MAX_METADATA_STRING_LENGTH: usize = 2048;

/// Which limit was exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScanLimitKind {
    /// More than [`MAX_SCAN_ENTRIES`] entries.
    EntryCount,
    /// A directory deeper than [`MAX_DIRECTORY_DEPTH`].
    DirectoryDepth,
    /// An `event.json` larger than [`MAX_EVENT_METADATA_SIZE`].
    MetadataSize,
    /// An `event.json` string longer than [`MAX_METADATA_STRING_LENGTH`].
    MetadataString,
}

impl ScanLimitKind {
    /// The contract id, e.g. `entry-count`.
    pub const fn id(self) -> &'static str {
        match self {
            ScanLimitKind::EntryCount => "entry-count",
            ScanLimitKind::DirectoryDepth => "directory-depth",
            ScanLimitKind::MetadataSize => "metadata-size",
            ScanLimitKind::MetadataString => "metadata-string",
        }
    }
}

/// A scan limit was exceeded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanLimitError {
    /// The exceeded limit.
    pub kind: ScanLimitKind,
}

impl ScanLimitError {
    /// An error of `kind`.
    pub const fn new(kind: ScanLimitKind) -> ScanLimitError {
        ScanLimitError { kind }
    }
}

impl fmt::Display for ScanLimitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Selected folder exceeds a safe local scan limit")
    }
}

impl std::error::Error for ScanLimitError {}

/// Fails when `count` entries exceed [`MAX_SCAN_ENTRIES`].
pub fn ensure_scan_entry_count(count: u64) -> Result<(), ScanLimitError> {
    if count > MAX_SCAN_ENTRIES {
        Err(ScanLimitError::new(ScanLimitKind::EntryCount))
    } else {
        Ok(())
    }
}

/// Fails when `depth` exceeds [`MAX_DIRECTORY_DEPTH`].
pub fn ensure_directory_depth(depth: usize) -> Result<(), ScanLimitError> {
    if depth > MAX_DIRECTORY_DEPTH {
        Err(ScanLimitError::new(ScanLimitKind::DirectoryDepth))
    } else {
        Ok(())
    }
}

/// Fails when `size` bytes exceed [`MAX_EVENT_METADATA_SIZE`].
pub fn ensure_metadata_size(size: u64) -> Result<(), ScanLimitError> {
    if size > MAX_EVENT_METADATA_SIZE {
        Err(ScanLimitError::new(ScanLimitKind::MetadataSize))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_are_inclusive() {
        assert!(ensure_scan_entry_count(MAX_SCAN_ENTRIES).is_ok());
        assert_eq!(
            ensure_scan_entry_count(MAX_SCAN_ENTRIES + 1).map_err(|error| error.kind),
            Err(ScanLimitKind::EntryCount)
        );
        assert!(ensure_directory_depth(8).is_ok());
        assert!(ensure_directory_depth(9).is_err());
        assert!(ensure_metadata_size(1_048_576).is_ok());
        assert!(ensure_metadata_size(1_048_577).is_err());
    }

    #[test]
    fn message_never_contains_paths() {
        let error = ScanLimitError::new(ScanLimitKind::DirectoryDepth);
        assert_eq!(
            error.to_string(),
            "Selected folder exceeds a safe local scan limit"
        );
        assert_eq!(error.kind.id(), "directory-depth");
    }
}
