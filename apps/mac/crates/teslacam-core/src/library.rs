//! Building the event library from the files of a scanned folder.
//!
//! Classification is pure ([`classify_files`]); reading `event.json` files is left to the caller
//! ([`ClassifiedFiles::finish`]) so the scanner can read them in parallel ([`crate::scan`]).

use std::collections::HashMap;
use std::path::PathBuf;

use crate::camera::{CameraId, CameraMap, ClipCategory};
use crate::filename::parse_clip_file_name;
use crate::limits::{ScanLimitError, ensure_scan_entry_count};
use crate::metadata::EventMetadata;
use crate::timestamp::{LocalTimestamp, parse_filename_local_timestamp};

/// How often progress is reported, in files.
pub const PROGRESS_INTERVAL: u64 = 250;
/// Recent segments further apart than this (in seconds) start a new session.
pub const RECENT_SESSION_GAP_SECONDS: i64 = 90;

/// A file collected by the folder scan. Its contents are not read during the scan.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileEntry {
    /// The entry name, e.g. `2026-08-21_14-30-45-front.mp4`.
    pub name: String,
    /// `/`-joined path from the selected folder's name, e.g. `TeslaCam/SavedClips/a/event.json`.
    pub relative_path: String,
    /// Byte size at scan time.
    pub size: u64,
    /// Absolute path for opening the file later (empty for in-memory inputs).
    pub path: PathBuf,
}

impl FileEntry {
    /// An entry without an absolute path.
    pub fn new(name: impl Into<String>, relative_path: impl Into<String>, size: u64) -> FileEntry {
        FileEntry {
            name: name.into(),
            relative_path: relative_path.into(),
            size,
            path: PathBuf::new(),
        }
    }
}

/// Index of a file in [`Library::files`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FileId(pub usize);

/// All clips that share one file-name timestamp inside one event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    /// The file-name timestamp, e.g. `2026-08-21_14-30-45`.
    pub timestamp: String,
    /// The clip of each camera; any camera may be missing.
    pub cameras: CameraMap<Option<FileId>>,
}

impl Segment {
    /// The parsed timestamp.
    pub fn local_timestamp(&self) -> Option<LocalTimestamp> {
        parse_filename_local_timestamp(&self.timestamp)
    }
}

/// An event: a Saved or Sentry directory, or a Recent session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Event {
    /// `{category}-{timestamp}-{rank}`, rank 1-based within the category.
    pub id: String,
    /// Where the clips came from.
    pub category: ClipCategory,
    /// The first segment's timestamp (never the `event.json` timestamp).
    pub timestamp: String,
    /// Segments in ascending time order.
    pub segments: Vec<Segment>,
    /// `event.mp4`, the single-stream representative video.
    pub representative: Option<FileId>,
    /// `thumb.png`.
    pub thumbnail: Option<FileId>,
    /// Parsed `event.json`.
    pub metadata: Option<EventMetadata>,
}

impl Event {
    /// The parsed start timestamp.
    pub fn start(&self) -> Option<LocalTimestamp> {
        parse_filename_local_timestamp(&self.timestamp)
    }
}

/// Why files were left out of the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WarningKind {
    /// A 0-byte `.mp4`.
    EmptyMediaFile,
    /// An `event.json` that cannot be read or validated.
    InvalidMetadataFile,
    /// Anything that is neither a clip nor a helper file.
    UnknownFile,
    /// A file inside `EncryptedClips`.
    EncryptedFile,
}

impl WarningKind {
    /// Display order.
    pub const ALL: [WarningKind; 4] = [
        WarningKind::EmptyMediaFile,
        WarningKind::InvalidMetadataFile,
        WarningKind::UnknownFile,
        WarningKind::EncryptedFile,
    ];

    /// The contract id, e.g. `unknown-file`.
    pub const fn id(self) -> &'static str {
        match self {
            WarningKind::EmptyMediaFile => "empty-media-file",
            WarningKind::InvalidMetadataFile => "invalid-metadata-file",
            WarningKind::UnknownFile => "unknown-file",
            WarningKind::EncryptedFile => "encrypted-file",
        }
    }

    const fn index(self) -> usize {
        match self {
            WarningKind::EmptyMediaFile => 0,
            WarningKind::InvalidMetadataFile => 1,
            WarningKind::UnknownFile => 2,
            WarningKind::EncryptedFile => 3,
        }
    }
}

/// A warning carries only a kind and a count: never paths, names or timestamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScanWarning {
    /// What was left out.
    pub kind: WarningKind,
    /// How many files (at least 1).
    pub count: u64,
}

/// Scan progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ScanProgress {
    /// Entries or files handled so far.
    pub processed: u64,
    /// The total when known; equal to `processed` during the walk.
    pub total: u64,
}

/// The events of a scanned folder.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Library {
    /// Every collected file; [`FileId`]s index into it.
    pub files: Vec<FileEntry>,
    /// All events, newest first. Ties keep Recent, Saved, Sentry order, then first-seen order.
    pub events: Vec<Event>,
    /// Warnings in display order, only kinds with a count above 0.
    pub warnings: Vec<ScanWarning>,
}

impl Library {
    /// The file behind `id`.
    pub fn file(&self, id: FileId) -> &FileEntry {
        &self.files[id.0]
    }

    /// The event with `id`.
    pub fn event(&self, id: &str) -> Option<&Event> {
        self.events.iter().find(|event| event.id == id)
    }

    /// The events of one category, newest first (the per-category order used for ids).
    pub fn events_in(&self, category: ClipCategory) -> impl Iterator<Item = &Event> {
        self.events
            .iter()
            .filter(move |event| event.category == category)
    }

    /// Number of events in `category`, independent of any filter.
    pub fn category_count(&self, category: ClipCategory) -> usize {
        self.events_in(category).count()
    }
}

enum FileLocation<'a> {
    Category(ClipCategory, Vec<&'a str>),
    Encrypted,
    Unknown,
}

fn locate(relative_path: &str) -> FileLocation<'_> {
    let parts = relative_path.split(['/', '\\']).collect::<Vec<_>>();
    for part in &parts {
        match *part {
            "RecentClips" => return FileLocation::Category(ClipCategory::Recent, parts),
            "SavedClips" => return FileLocation::Category(ClipCategory::Saved, parts),
            "SentryClips" => return FileLocation::Category(ClipCategory::Sentry, parts),
            "EncryptedClips" => return FileLocation::Encrypted,
            _ => {}
        }
    }
    FileLocation::Unknown
}

fn directory_key(category: ClipCategory, parts: &[&str]) -> Option<String> {
    let index = parts
        .iter()
        .position(|part| *part == category.folder_name())?;
    let parent = parts.get(index + 1)?;
    Some(format!("{}:{parent}", category.id()))
}

fn is_empty_media(file: &FileEntry) -> bool {
    file.name.ends_with(".mp4") && file.size == 0
}

#[derive(Debug, Default)]
struct DirectoryEvent {
    category: Option<ClipCategory>,
    clips: HashMap<String, CameraMap<Option<FileId>>>,
    representative: Option<FileId>,
    thumbnail: Option<FileId>,
    metadata_file: Option<FileId>,
}

/// The result of classifying every file; finish it with [`ClassifiedFiles::finish`].
#[derive(Debug)]
pub struct ClassifiedFiles {
    files: Vec<FileEntry>,
    directory_order: Vec<String>,
    directory_events: HashMap<String, DirectoryEvent>,
    recent_clips: HashMap<String, CameraMap<Option<FileId>>>,
    warning_counts: [u64; 4],
}

/// Classifies `files` (in collection order) into clips, helpers and warnings.
///
/// Fails with [`crate::limits::ScanLimitKind::EntryCount`] for more than 100 000 files. Progress is
/// reported after every 250th file and once at the end unless the count is a multiple of 250.
pub fn classify_files(
    files: Vec<FileEntry>,
    mut progress: impl FnMut(ScanProgress),
) -> Result<ClassifiedFiles, ScanLimitError> {
    let total = files.len() as u64;
    ensure_scan_entry_count(total)?;
    let mut classified = ClassifiedFiles {
        files: Vec::new(),
        directory_order: Vec::new(),
        directory_events: HashMap::new(),
        recent_clips: HashMap::new(),
        warning_counts: [0; 4],
    };
    for (index, file) in files.iter().enumerate() {
        classified.classify(FileId(index), file);
        let processed = index as u64 + 1;
        if processed.is_multiple_of(PROGRESS_INTERVAL) {
            progress(ScanProgress { processed, total });
        }
    }
    if !total.is_multiple_of(PROGRESS_INTERVAL) {
        progress(ScanProgress {
            processed: total,
            total,
        });
    }
    classified.files = files;
    Ok(classified)
}

impl ClassifiedFiles {
    fn warn(&mut self, kind: WarningKind) {
        self.warning_counts[kind.index()] += 1;
    }

    fn directory_event(&mut self, key: String, category: ClipCategory) -> &mut DirectoryEvent {
        if !self.directory_events.contains_key(&key) {
            self.directory_order.push(key.clone());
        }
        let event = self.directory_events.entry(key).or_default();
        event.category.get_or_insert(category);
        event
    }

    fn classify(&mut self, id: FileId, file: &FileEntry) {
        let (category, parts) = match locate(&file.relative_path) {
            FileLocation::Encrypted => return self.warn(WarningKind::EncryptedFile),
            FileLocation::Unknown if is_empty_media(file) => {
                return self.warn(WarningKind::EmptyMediaFile);
            }
            FileLocation::Unknown => return self.warn(WarningKind::UnknownFile),
            FileLocation::Category(category, parts) => (category, parts),
        };
        if is_empty_media(file) {
            return self.warn(WarningKind::EmptyMediaFile);
        }
        let helper = matches!(file.name.as_str(), "event.mp4" | "event.json" | "thumb.png");
        if category != ClipCategory::Recent && helper {
            let Some(key) = directory_key(category, &parts) else {
                return self.warn(WarningKind::UnknownFile);
            };
            let event = self.directory_event(key, category);
            match file.name.as_str() {
                "event.mp4" => event.representative = Some(id),
                "thumb.png" => event.thumbnail = Some(id),
                _ => event.metadata_file = Some(id),
            }
            return;
        }
        let Some(clip) = parse_clip_file_name(&file.name) else {
            return self.warn(WarningKind::UnknownFile);
        };
        if category == ClipCategory::Recent {
            self.recent_clips.entry(clip.timestamp).or_default()[clip.camera] = Some(id);
            return;
        }
        let Some(key) = directory_key(category, &parts) else {
            return self.warn(WarningKind::UnknownFile);
        };
        self.directory_event(key, category)
            .clips
            .entry(clip.timestamp)
            .or_default()[clip.camera] = Some(id);
    }

    /// The collected files.
    pub fn files(&self) -> &[FileEntry] {
        &self.files
    }

    /// The `event.json` files that [`Self::finish`] will read: those of directory events with at
    /// least one segment, in first-seen order.
    pub fn metadata_requests(&self) -> Vec<FileId> {
        self.directory_order
            .iter()
            .filter_map(|key| {
                let event = &self.directory_events[key];
                (!event.clips.is_empty())
                    .then_some(event.metadata_file)
                    .flatten()
            })
            .collect()
    }

    /// Groups the classified files into events.
    ///
    /// `read_metadata` is called once per [`Self::metadata_requests`] entry, in that order; an error
    /// aborts the build, `Ok(None)` adds an `invalid-metadata-file` warning.
    pub fn finish(
        mut self,
        mut read_metadata: impl FnMut(
            FileId,
            &FileEntry,
        ) -> Result<Option<EventMetadata>, ScanLimitError>,
    ) -> Result<Library, ScanLimitError> {
        let mut by_category: EventsByCategory = Default::default();
        for key in std::mem::take(&mut self.directory_order) {
            let Some(directory) = self.directory_events.remove(&key) else {
                continue;
            };
            let segments = sorted_segments(directory.clips);
            let Some(first) = segments.first() else {
                continue;
            };
            let metadata = match directory.metadata_file {
                None => None,
                Some(file) => {
                    let metadata = read_metadata(file, &self.files[file.0])?;
                    if metadata.is_none() {
                        self.warn(WarningKind::InvalidMetadataFile);
                    }
                    metadata
                }
            };
            let category = directory.category.unwrap_or(ClipCategory::Saved);
            by_category.push(Event {
                id: String::new(),
                category,
                timestamp: first.timestamp.clone(),
                segments,
                representative: directory.representative,
                thumbnail: directory.thumbnail,
                metadata,
            });
        }
        for session in recent_sessions(sorted_segments(std::mem::take(&mut self.recent_clips))) {
            by_category.push(Event {
                id: String::new(),
                category: ClipCategory::Recent,
                timestamp: session[0].timestamp.clone(),
                segments: session,
                representative: None,
                thumbnail: None,
                metadata: None,
            });
        }
        let mut events = Vec::new();
        for category in ClipCategory::ALL {
            events.extend(ranked(by_category.take(category)));
        }
        events.sort_by_key(|event| std::cmp::Reverse(timestamp_value(&event.timestamp)));
        let warnings = WarningKind::ALL
            .into_iter()
            .filter(|kind| self.warning_counts[kind.index()] > 0)
            .map(|kind| ScanWarning {
                kind,
                count: self.warning_counts[kind.index()],
            })
            .collect();
        Ok(Library {
            files: self.files,
            events,
            warnings,
        })
    }
}

/// Events collected per category before ranking.
#[derive(Default)]
struct EventsByCategory {
    recent: Vec<Event>,
    saved: Vec<Event>,
    sentry: Vec<Event>,
}

impl EventsByCategory {
    fn push(&mut self, event: Event) {
        match event.category {
            ClipCategory::Recent => self.recent.push(event),
            ClipCategory::Saved => self.saved.push(event),
            ClipCategory::Sentry => self.sentry.push(event),
        }
    }

    fn take(&mut self, category: ClipCategory) -> Vec<Event> {
        std::mem::take(match category {
            ClipCategory::Recent => &mut self.recent,
            ClipCategory::Saved => &mut self.saved,
            ClipCategory::Sentry => &mut self.sentry,
        })
    }
}

/// Ordinal seconds of a file-name timestamp (0 when invalid, as on the web).
fn timestamp_value(timestamp: &str) -> i64 {
    parse_filename_local_timestamp(timestamp).map_or(0, |timestamp| timestamp.ordinal_seconds())
}

fn sorted_segments(clips: HashMap<String, CameraMap<Option<FileId>>>) -> Vec<Segment> {
    let mut segments = clips
        .into_iter()
        .map(|(timestamp, cameras)| Segment { timestamp, cameras })
        .collect::<Vec<_>>();
    segments.sort_by_key(|segment| timestamp_value(&segment.timestamp));
    segments
}

fn recent_sessions(segments: Vec<Segment>) -> Vec<Vec<Segment>> {
    let mut sessions: Vec<Vec<Segment>> = Vec::new();
    for segment in segments {
        let joins = sessions
            .last()
            .and_then(|session| session.last())
            .is_some_and(|previous| {
                timestamp_value(&segment.timestamp) - timestamp_value(&previous.timestamp)
                    <= RECENT_SESSION_GAP_SECONDS
            });
        match sessions.last_mut() {
            Some(session) if joins => session.push(segment),
            _ => sessions.push(vec![segment]),
        }
    }
    sessions
}

fn ranked(mut events: Vec<Event>) -> Vec<Event> {
    events.sort_by_key(|event| std::cmp::Reverse(timestamp_value(&event.timestamp)));
    for (index, event) in events.iter_mut().enumerate() {
        event.id = format!("{}-{}-{}", event.category.id(), event.timestamp, index + 1);
    }
    events
}

/// Classifies `files` and builds the library, reading metadata sequentially with `read_metadata`.
pub fn build_library(
    files: Vec<FileEntry>,
    progress: impl FnMut(ScanProgress),
    read_metadata: impl FnMut(FileId, &FileEntry) -> Result<Option<EventMetadata>, ScanLimitError>,
) -> Result<Library, ScanLimitError> {
    classify_files(files, progress)?.finish(read_metadata)
}

/// The cameras of a segment that have a clip, in canonical order.
pub fn segment_cameras(segment: &Segment) -> Vec<CameraId> {
    segment.cameras.present()
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value, json};

    use super::*;
    use crate::contract::{metadata_json, run_fixture, string_input};
    use crate::metadata::read_event_metadata;

    const DEFAULT_CONTENTS: &str = "synthetic-file";

    struct Input {
        files: Vec<FileEntry>,
        contents: Vec<Vec<u8>>,
    }

    fn input(value: &Value) -> Input {
        let mut files = Vec::new();
        let mut contents = Vec::new();
        let mut push = |file: &Value| {
            let relative_path = file["relativePath"]
                .as_str()
                .expect("relativePath")
                .to_owned();
            let name = file.get("name").and_then(Value::as_str).map_or_else(
                || {
                    relative_path
                        .rsplit('/')
                        .next()
                        .unwrap_or(&relative_path)
                        .to_owned()
                },
                str::to_owned,
            );
            let bytes = file.get("text").map_or_else(
                || DEFAULT_CONTENTS.as_bytes().to_vec(),
                |text| string_input(text).into_bytes(),
            );
            let size = file
                .get("size")
                .and_then(Value::as_u64)
                .unwrap_or(bytes.len() as u64);
            files.push(FileEntry::new(name, relative_path, size));
            contents.push(bytes);
        };
        for file in value["files"].as_array().expect("files") {
            push(file);
        }
        if let Some(generate) = value.get("generate") {
            let pattern = generate["pathPattern"].as_str().expect("pathPattern");
            for index in 0..generate["count"].as_u64().expect("count") {
                let mut file = Map::new();
                file.insert(
                    "relativePath".into(),
                    Value::from(pattern.replace("{i}", &index.to_string())),
                );
                if let Some(size) = generate.get("size") {
                    file.insert("size".into(), size.clone());
                }
                push(&Value::Object(file));
            }
        }
        Input { files, contents }
    }

    fn event_json(library: &Library, event: &Event) -> Value {
        let path = |id: Option<FileId>| {
            id.map_or(Value::Null, |id| {
                Value::from(library.file(id).relative_path.clone())
            })
        };
        json!({
            "id": event.id,
            "category": event.category.id(),
            "timestamp": event.timestamp,
            "segments": event.segments.iter().map(|segment| {
                let cameras = segment.cameras.iter()
                    .filter_map(|(camera, file)| file.map(|file| (camera.id().to_owned(), Value::from(library.file(file).relative_path.clone()))))
                    .collect::<Map<_, _>>();
                json!({"timestamp": segment.timestamp, "cameras": cameras})
            }).collect::<Vec<_>>(),
            "representative": path(event.representative),
            "thumbnail": path(event.thumbnail),
            "metadata": event.metadata.as_ref().map_or(Value::Null, metadata_json),
        })
    }

    fn library_json(case_input: &Value) -> Value {
        let Input { files, contents } = input(case_input);
        let mut progress = Vec::new();
        let result = build_library(
            files,
            |update| progress.push(json!({"processed": update.processed, "total": update.total})),
            |id, file| {
                let bytes = &contents[id.0];
                read_event_metadata(file.size, |limit| {
                    Ok(bytes.iter().copied().take(limit as usize).collect())
                })
            },
        );
        match result {
            Err(error) => json!({"error": "scan-limit", "kind": error.kind.id()}),
            Ok(library) => {
                let ids = |category| {
                    library
                        .events_in(category)
                        .map(|event| event.id.clone())
                        .collect::<Vec<_>>()
                };
                json!({
                    "events": library.events.iter().map(|event| event_json(&library, event)).collect::<Vec<_>>(),
                    "eventsByCategory": {
                        "recent": ids(ClipCategory::Recent),
                        "saved": ids(ClipCategory::Saved),
                        "sentry": ids(ClipCategory::Sentry),
                    },
                    "warnings": library.warnings.iter()
                        .map(|warning| json!({"kind": warning.kind.id(), "count": warning.count}))
                        .collect::<Vec<_>>(),
                    "progress": progress,
                })
            }
        }
    }

    #[test]
    fn matches_the_library_build_fixture() {
        run_fixture("library-build", |case| match case.function.as_str() {
            "buildTeslaCamLibrary" => Some(library_json(&case.input)),
            _ => None,
        });
    }

    fn clip(path: &str) -> FileEntry {
        let name = path.rsplit('/').next().expect("name");
        FileEntry::new(name, path, 10)
    }

    #[test]
    fn metadata_is_requested_only_for_events_with_segments() {
        let files = vec![
            clip("TeslaCam/SentryClips/a/event.json"),
            clip("TeslaCam/SentryClips/b/event.json"),
            clip("TeslaCam/SentryClips/b/2026-08-21_10-00-00-front.mp4"),
        ];
        let classified = classify_files(files, |_| {}).expect("classified");
        assert_eq!(classified.metadata_requests(), vec![FileId(1)]);
        let mut reads = Vec::new();
        let library = classified
            .finish(|id, _| {
                reads.push(id);
                Ok(None)
            })
            .expect("library");
        assert_eq!(reads, vec![FileId(1)]);
        assert_eq!(
            library.warnings,
            vec![ScanWarning {
                kind: WarningKind::InvalidMetadataFile,
                count: 1
            }]
        );
    }

    #[test]
    fn a_metadata_limit_error_aborts_the_build() {
        let files = vec![
            clip("TeslaCam/SentryClips/a/2026-08-21_10-00-00-front.mp4"),
            clip("TeslaCam/SentryClips/a/event.json"),
        ];
        let result = build_library(
            files,
            |_| {},
            |_, _| {
                Err(ScanLimitError::new(
                    crate::limits::ScanLimitKind::MetadataString,
                ))
            },
        );
        assert_eq!(
            result.map(|_| ()).map_err(|error| error.kind),
            Err(crate::limits::ScanLimitKind::MetadataString)
        );
    }

    #[test]
    fn category_counts_ignore_filters() {
        let files = vec![
            clip("TeslaCam/SentryClips/a/2026-08-21_10-00-00-front.mp4"),
            clip("TeslaCam/SavedClips/b/2026-08-21_11-00-00-front.mp4"),
            clip("TeslaCam/SavedClips/c/2026-08-21_12-00-00-front.mp4"),
        ];
        let library = build_library(files, |_| {}, |_, _| Ok(None)).expect("library");
        assert_eq!(library.category_count(ClipCategory::Saved), 2);
        assert_eq!(library.category_count(ClipCategory::Recent), 0);
        assert_eq!(
            library
                .event("saved-2026-08-21_12-00-00-1")
                .map(|event| event.segments.len()),
            Some(1)
        );
        assert_eq!(
            segment_cameras(&library.events[0].segments[0]),
            vec![CameraId::Front]
        );
    }
}
