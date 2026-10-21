//! Walking a selected folder and building its library (std::fs only).
//!
//! The walk mirrors the web's `pickTeslaCamDirectory`: depth-first, every enumerated entry counts
//! against [`MAX_SCAN_ENTRIES`](crate::limits::MAX_SCAN_ENTRIES), entries whose name starts with `.`
//! are counted but skipped, and entering a directory deeper than
//! [`MAX_DIRECTORY_DEPTH`](crate::limits::MAX_DIRECTORY_DEPTH) fails. Mac decisions: entries are
//! listed in byte-wise name order, and symbolic links (and anything else that is neither a regular
//! file nor a directory) are counted but neither followed nor collected.

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::library::{FileEntry, FileId, Library, PROGRESS_INTERVAL, ScanProgress, classify_files};
use crate::limits::{
    ScanLimitError, ScanLimitKind, ensure_directory_depth, ensure_scan_entry_count,
};
use crate::metadata::{EventMetadata, read_event_metadata};

/// Maximum threads reading `event.json` files at once.
pub const METADATA_READERS: usize = 4;

/// What kind of entry a directory listing returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    /// A regular file with its byte size.
    File {
        /// Byte size.
        size: u64,
    },
    /// A directory (walked recursively).
    Directory,
    /// A symbolic link or another special entry (counted, never followed).
    Other,
}

/// One entry of a directory listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryEntry<H> {
    /// The entry name.
    pub name: String,
    /// What it is.
    pub kind: EntryKind,
    /// How to reach it (a path for the file system).
    pub handle: H,
}

/// A tree that can be walked: the file system ([`FileSystem`]) or an in-memory tree in tests.
pub trait DirectorySource {
    /// Identifies a directory or file.
    type Handle;

    /// The entries of `directory`, in the order the walk visits them.
    fn entries(&self, directory: &Self::Handle) -> io::Result<Vec<DirectoryEntry<Self::Handle>>>;

    /// The absolute path stored in [`FileEntry::path`] for a file.
    fn file_path(&self, file: &Self::Handle) -> PathBuf;
}

/// The local file system. Entries are sorted by name (byte-wise); symbolic links are not followed.
#[derive(Debug, Clone, Copy, Default)]
pub struct FileSystem;

impl DirectorySource for FileSystem {
    type Handle = PathBuf;

    fn entries(&self, directory: &PathBuf) -> io::Result<Vec<DirectoryEntry<PathBuf>>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            let kind = if file_type.is_file() {
                EntryKind::File {
                    size: entry.metadata()?.len(),
                }
            } else if file_type.is_dir() {
                EntryKind::Directory
            } else {
                EntryKind::Other
            };
            entries.push((entry.file_name(), kind, entry.path()));
        }
        entries.sort_by(|left, right| left.0.as_encoded_bytes().cmp(right.0.as_encoded_bytes()));
        Ok(entries
            .into_iter()
            .map(|(name, kind, handle)| DirectoryEntry {
                name: name.to_string_lossy().into_owned(),
                kind,
                handle,
            })
            .collect())
    }

    fn file_path(&self, file: &PathBuf) -> PathBuf {
        file.clone()
    }
}

/// Why a scan stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScanError {
    /// The cancellation flag was set.
    Cancelled,
    /// Reading the selected folder itself was refused (sandbox, TCC, permissions).
    Denied,
    /// A safe local scan limit was exceeded.
    Limit(ScanLimitKind),
    /// Any other I/O error.
    Io(io::ErrorKind),
}

impl From<ScanLimitError> for ScanError {
    fn from(error: ScanLimitError) -> ScanError {
        ScanError::Limit(error.kind)
    }
}

/// The files collected by a walk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkedFiles {
    /// The selected folder's display name.
    pub folder_name: String,
    /// Regular, non-hidden files in visiting order.
    pub files: Vec<FileEntry>,
    /// Every enumerated entry, hidden and special ones included.
    pub processed: u64,
}

struct Walk<'a, S: DirectorySource> {
    source: &'a S,
    progress: &'a mut dyn FnMut(ScanProgress),
    cancel: &'a AtomicBool,
    files: Vec<FileEntry>,
    processed: u64,
}

impl<S: DirectorySource> Walk<'_, S> {
    fn directory(
        &mut self,
        handle: &S::Handle,
        path: &mut Vec<String>,
        depth: usize,
    ) -> Result<(), ScanError> {
        ensure_directory_depth(depth)?;
        let entries = self.source.entries(handle).map_err(|error| {
            if depth == 0 && error.kind() == io::ErrorKind::PermissionDenied {
                ScanError::Denied
            } else {
                ScanError::Io(error.kind())
            }
        })?;
        for entry in entries {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(ScanError::Cancelled);
            }
            self.processed += 1;
            ensure_scan_entry_count(self.processed)?;
            if self.processed.is_multiple_of(PROGRESS_INTERVAL) {
                (self.progress)(ScanProgress {
                    processed: self.processed,
                    total: self.processed,
                });
            }
            if entry.name.starts_with('.') {
                continue;
            }
            match entry.kind {
                EntryKind::File { size } => {
                    let relative_path = path
                        .iter()
                        .map(String::as_str)
                        .chain([entry.name.as_str()])
                        .collect::<Vec<_>>()
                        .join("/");
                    self.files.push(FileEntry {
                        path: self.source.file_path(&entry.handle),
                        name: entry.name,
                        relative_path,
                        size,
                    });
                }
                EntryKind::Directory => {
                    path.push(entry.name);
                    self.directory(&entry.handle, path, depth + 1)?;
                    path.pop();
                }
                EntryKind::Other => {}
            }
        }
        Ok(())
    }
}

/// Walks `root`, whose display name is `folder_name`. Progress is reported every 250 entries as
/// `{processed, total: processed}` and once more at the end.
pub fn walk<S: DirectorySource>(
    source: &S,
    root: &S::Handle,
    folder_name: &str,
    progress: &mut dyn FnMut(ScanProgress),
    cancel: &AtomicBool,
) -> Result<WalkedFiles, ScanError> {
    let mut walk = Walk {
        source,
        progress,
        cancel,
        files: Vec::new(),
        processed: 0,
    };
    walk.directory(root, &mut vec![folder_name.to_owned()], 0)?;
    let processed = walk.processed;
    (walk.progress)(ScanProgress {
        processed,
        total: processed,
    });
    Ok(WalkedFiles {
        folder_name: folder_name.to_owned(),
        files: walk.files,
        processed,
    })
}

/// The display name of a selected folder: its last path component (a volume's name for a volume root).
pub fn folder_name(root: &Path) -> String {
    root.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.to_string_lossy().into_owned())
}

/// Walks a folder on disk.
pub fn walk_directory(
    root: &Path,
    progress: &mut dyn FnMut(ScanProgress),
    cancel: &AtomicBool,
) -> Result<WalkedFiles, ScanError> {
    walk(
        &FileSystem,
        &root.to_path_buf(),
        &folder_name(root),
        progress,
        cancel,
    )
}

fn read_metadata_file(file: &FileEntry) -> Result<Option<EventMetadata>, ScanLimitError> {
    read_event_metadata(file.size, |limit| {
        let mut bytes = Vec::new();
        File::open(&file.path)?
            .take(limit)
            .read_to_end(&mut bytes)?;
        Ok(bytes)
    })
}

/// Reads the requested `event.json` files on up to `workers` threads.
pub fn read_metadata_files(
    files: &[FileEntry],
    requests: &[FileId],
    workers: usize,
) -> HashMap<FileId, Result<Option<EventMetadata>, ScanLimitError>> {
    let next = AtomicUsize::new(0);
    let results = Mutex::new(HashMap::with_capacity(requests.len()));
    std::thread::scope(|scope| {
        for _ in 0..workers.clamp(1, requests.len().max(1)) {
            scope.spawn(|| {
                while let Some(id) = requests.get(next.fetch_add(1, Ordering::Relaxed)) {
                    let result = read_metadata_file(&files[id.0]);
                    results
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .insert(*id, result);
                }
            });
        }
    });
    results
        .into_inner()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// A scanned folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedFolder {
    /// The folder's display name.
    pub folder_name: String,
    /// Its library (possibly without events: the app shows the "invalid" screen then).
    pub library: Library,
}

/// Walks `root`, classifies the files and reads `event.json` files in parallel.
///
/// Progress: the walk's reports (`total == processed`), then classification reports
/// (`processed / total` over the collected files).
pub fn scan_library(
    root: &Path,
    progress: &mut dyn FnMut(ScanProgress),
    cancel: &AtomicBool,
) -> Result<ScannedFolder, ScanError> {
    let walked = walk_directory(root, progress, cancel)?;
    let classified = classify_files(walked.files, &mut *progress)?;
    if cancel.load(Ordering::Relaxed) {
        return Err(ScanError::Cancelled);
    }
    let requests = classified.metadata_requests();
    let mut metadata = read_metadata_files(classified.files(), &requests, METADATA_READERS);
    if cancel.load(Ordering::Relaxed) {
        return Err(ScanError::Cancelled);
    }
    let library = classified.finish(|id, _| metadata.remove(&id).unwrap_or(Ok(None)))?;
    Ok(ScannedFolder {
        folder_name: walked.folder_name,
        library,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicU64;

    use serde_json::{Value, json};

    use super::*;
    use crate::camera::{CameraId, ClipCategory};
    use crate::contract::run_fixture;
    use crate::library::WarningKind;

    /// An in-memory tree built from a `directory-walk.json` node.
    struct MemoryTree;

    impl DirectorySource for MemoryTree {
        type Handle = Value;

        fn entries(&self, directory: &Value) -> io::Result<Vec<DirectoryEntry<Value>>> {
            let mut entries = directory
                .get("entries")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            if let Some(generate) = directory.get("generate") {
                let pattern = generate["namePattern"].as_str().expect("namePattern");
                let size = generate.get("size").and_then(Value::as_u64).unwrap_or(1);
                for index in 0..generate["count"].as_u64().expect("count") {
                    entries.push(json!({"kind": "file", "name": pattern.replace("{i}", &index.to_string()), "size": size}));
                }
            }
            Ok(entries
                .into_iter()
                .map(|node| DirectoryEntry {
                    name: node["name"].as_str().expect("name").to_owned(),
                    kind: match node["kind"].as_str().expect("kind") {
                        "file" => EntryKind::File {
                            size: node["size"].as_u64().expect("size"),
                        },
                        "directory" => EntryKind::Directory,
                        _ => EntryKind::Other,
                    },
                    handle: node,
                })
                .collect())
        }

        fn file_path(&self, _file: &Value) -> PathBuf {
            PathBuf::new()
        }
    }

    #[test]
    fn matches_the_directory_walk_fixture() {
        run_fixture("directory-walk", |case| match case.function.as_str() {
            "walkDirectory" => {
                let root = case.input("root");
                let mut progress = Vec::new();
                let result = walk(
                    &MemoryTree,
                    root,
                    root["name"].as_str().expect("root name"),
                    &mut |update| {
                        progress.push(json!({"processed": update.processed, "total": update.total}))
                    },
                    &AtomicBool::new(false),
                );
                Some(match result {
                    Ok(walked) => json!({
                        "status": "selected",
                        "folderName": walked.folder_name,
                        "files": walked.files.iter().map(|file| json!({"name": file.name, "relativePath": file.relative_path, "size": file.size})).collect::<Vec<_>>(),
                        "progress": progress,
                    }),
                    Err(ScanError::Limit(kind)) => {
                        json!({"status": "scan-limit", "kind": kind.id()})
                    }
                    Err(error) => json!({"status": format!("{error:?}")}),
                })
            }
            _ => None,
        });
    }

    /// A temporary directory removed on drop.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> TempDir {
            static COUNTER: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "teslacam-core-{}-{}",
                std::process::id(),
                COUNTER.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).expect("temp dir");
            TempDir(path.canonicalize().expect("canonical temp dir"))
        }

        fn file(&self, relative: &str, contents: &[u8]) -> PathBuf {
            let path = self.0.join(relative);
            fs::create_dir_all(path.parent().expect("parent")).expect("directories");
            fs::write(&path, contents).expect("write");
            path
        }

        fn directory(&self, relative: &str) -> PathBuf {
            let path = self.0.join(relative);
            fs::create_dir_all(&path).expect("directories");
            path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn walk_paths(root: &Path) -> Result<(Vec<String>, Vec<ScanProgress>), ScanError> {
        let mut progress = Vec::new();
        let walked = walk_directory(
            root,
            &mut |update| progress.push(update),
            &AtomicBool::new(false),
        )?;
        Ok((
            walked
                .files
                .into_iter()
                .map(|file| file.relative_path)
                .collect(),
            progress,
        ))
    }

    #[test]
    fn walks_in_name_order_and_skips_hidden_entries() {
        let temp = TempDir::new();
        let root = temp.directory("TeslaCam");
        temp.file(
            "TeslaCam/SentryClips/b/2026-08-21_10-00-00-front.mp4",
            b"clip",
        );
        temp.file(
            "TeslaCam/SentryClips/a/2026-08-21_10-00-00-front.mp4",
            b"clip",
        );
        temp.file(
            "TeslaCam/RecentClips/2026-08-21_10-00-00-front.mp4",
            b"clip",
        );
        temp.file(
            "TeslaCam/SavedClips/.hidden/2026-08-21_10-00-00-front.mp4",
            b"clip",
        );
        temp.file("TeslaCam/.DS_Store", b"x");
        temp.file(
            "TeslaCam/SavedClips/a/._2026-08-21_10-00-00-front.mp4",
            b"x",
        );
        temp.file("TeslaCam/Z.txt", b"");
        temp.file("TeslaCam/a.txt", b"12345");
        let (paths, progress) = walk_paths(&root).expect("walk");
        assert_eq!(
            paths,
            [
                "TeslaCam/RecentClips/2026-08-21_10-00-00-front.mp4",
                "TeslaCam/SentryClips/a/2026-08-21_10-00-00-front.mp4",
                "TeslaCam/SentryClips/b/2026-08-21_10-00-00-front.mp4",
                "TeslaCam/Z.txt",
                "TeslaCam/a.txt",
            ]
        );
        let total = progress.last().expect("final progress");
        assert_eq!(total.processed, total.total);
        assert_eq!(total.processed, 14);
    }

    #[test]
    fn records_names_sizes_and_absolute_paths() {
        let temp = TempDir::new();
        let root = temp.directory("RecentClips");
        let clip = temp.file("RecentClips/2026-08-21_10-00-00-back.mp4", b"0123456789");
        let walked = walk_directory(&root, &mut |_| {}, &AtomicBool::new(false)).expect("walk");
        assert_eq!(walked.folder_name, "RecentClips");
        assert_eq!(
            walked.files,
            vec![FileEntry {
                name: "2026-08-21_10-00-00-back.mp4".into(),
                relative_path: "RecentClips/2026-08-21_10-00-00-back.mp4".into(),
                size: 10,
                path: clip,
            }]
        );
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_links_are_counted_but_not_followed() {
        let temp = TempDir::new();
        let root = temp.directory("TeslaCam");
        let outside = temp.directory("outside");
        temp.file("outside/2026-08-21_10-00-00-front.mp4", b"clip");
        temp.file(
            "TeslaCam/SavedClips/a/2026-08-21_10-00-00-back.mp4",
            b"clip",
        );
        std::os::unix::fs::symlink(&outside, root.join("RecentClips")).expect("directory link");
        std::os::unix::fs::symlink(
            outside.join("2026-08-21_10-00-00-front.mp4"),
            root.join("SavedClips/a/2026-08-21_10-00-00-front.mp4"),
        )
        .expect("file link");
        std::os::unix::fs::symlink(&root, root.join("loop")).expect("loop link");
        let mut progress = Vec::new();
        let walked = walk_directory(
            &root,
            &mut |update| progress.push(update),
            &AtomicBool::new(false),
        )
        .expect("walk");
        assert_eq!(
            walked
                .files
                .iter()
                .map(|file| file.relative_path.as_str())
                .collect::<Vec<_>>(),
            ["TeslaCam/SavedClips/a/2026-08-21_10-00-00-back.mp4"]
        );
        assert_eq!(walked.processed, 6);
    }

    #[test]
    fn depth_limit() {
        let temp = TempDir::new();
        let allowed = (0..=8)
            .map(|depth| format!("d{depth}"))
            .collect::<Vec<_>>()
            .join("/");
        temp.file(&format!("{allowed}/clip.mp4"), b"clip");
        assert_eq!(
            walk_paths(&temp.0.join("d0")).map(|(paths, _)| paths.len()),
            Ok(1)
        );
        temp.directory(&format!("{allowed}/d9"));
        assert_eq!(
            walk_paths(&temp.0.join("d0")).map(|_| ()),
            Err(ScanError::Limit(ScanLimitKind::DirectoryDepth))
        );
    }

    #[test]
    fn progress_every_250_entries() {
        let temp = TempDir::new();
        let root = temp.directory("RecentClips");
        for index in 0..501 {
            temp.file(&format!("RecentClips/.f{index:03}"), b"");
        }
        let (paths, progress) = walk_paths(&root).expect("walk");
        assert!(paths.is_empty());
        assert_eq!(
            progress,
            [
                ScanProgress {
                    processed: 250,
                    total: 250
                },
                ScanProgress {
                    processed: 500,
                    total: 500
                },
                ScanProgress {
                    processed: 501,
                    total: 501
                },
            ]
        );
    }

    #[test]
    fn cancellation_stops_the_walk() {
        let temp = TempDir::new();
        let root = temp.directory("RecentClips");
        for index in 0..300 {
            temp.file(&format!("RecentClips/{index:03}.txt"), b"");
        }
        let cancel = AtomicBool::new(true);
        assert_eq!(
            walk_directory(&root, &mut |_| {}, &cancel).map(|_| ()),
            Err(ScanError::Cancelled)
        );
        let cancel = AtomicBool::new(false);
        let result = walk_directory(
            &root,
            &mut |_| cancel.store(true, Ordering::Relaxed),
            &cancel,
        );
        assert_eq!(result.map(|_| ()), Err(ScanError::Cancelled));
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_root_is_denied() {
        use std::os::unix::fs::PermissionsExt;

        let temp = TempDir::new();
        let root = temp.directory("Locked");
        fs::set_permissions(&root, fs::Permissions::from_mode(0o000)).expect("lock");
        let result = walk_directory(&root, &mut |_| {}, &AtomicBool::new(false));
        fs::set_permissions(&root, fs::Permissions::from_mode(0o755)).expect("unlock");
        if fs::read_dir("/var/root").is_ok() {
            return; // running with elevated rights: permissions are not enforced
        }
        assert_eq!(result.map(|_| ()), Err(ScanError::Denied));
    }

    #[test]
    fn a_missing_root_fails() {
        let temp = TempDir::new();
        let result = walk_directory(
            &temp.0.join("missing"),
            &mut |_| {},
            &AtomicBool::new(false),
        );
        assert_eq!(
            result.map(|_| ()),
            Err(ScanError::Io(io::ErrorKind::NotFound))
        );
    }

    #[test]
    fn scans_a_teslacam_folder_with_parallel_metadata() {
        let temp = TempDir::new();
        let root = temp.directory("TeslaCam");
        for (directory, minute) in [
            ("a", "10"),
            ("b", "11"),
            ("c", "12"),
            ("d", "13"),
            ("e", "14"),
        ] {
            for camera in CameraId::ALL {
                temp.file(
                    &format!(
                        "TeslaCam/SentryClips/{directory}/2026-08-21_{minute}-00-00-{}.mp4",
                        camera.id()
                    ),
                    b"clip",
                );
            }
            temp.file(
                &format!("TeslaCam/SentryClips/{directory}/event.json"),
                format!(r#"{{"timestamp": "2026-08-21T{minute}:00:30", "reason": "sentry_aware_object_detection"}}"#).as_bytes(),
            );
        }
        temp.file("TeslaCam/SentryClips/e/thumb.png", b"png");
        temp.file(
            "TeslaCam/RecentClips/2026-08-21_09-00-00-front.mp4",
            b"clip",
        );
        temp.file("TeslaCam/RecentClips/2026-08-21_09-01-00-front.mp4", b"");
        temp.file("TeslaCam/EncryptedClips/x.mp4", b"x");
        let mut progress = Vec::new();
        let scanned = scan_library(
            &root,
            &mut |update| progress.push(update),
            &AtomicBool::new(false),
        )
        .expect("scan");
        let library = scanned.library;
        assert_eq!(scanned.folder_name, "TeslaCam");
        assert_eq!(library.category_count(ClipCategory::Sentry), 5);
        assert_eq!(library.category_count(ClipCategory::Recent), 1);
        assert!(
            library
                .events_in(ClipCategory::Sentry)
                .all(|event| event.metadata.is_some())
        );
        let newest = &library.events[0];
        assert_eq!(newest.id, "sentry-2026-08-21_14-00-00-1");
        assert_eq!(
            newest
                .metadata
                .as_ref()
                .map(|metadata| metadata.timestamp.as_str()),
            Some("2026-08-21T14:00:30")
        );
        let thumbnail = newest.thumbnail.map(|id| library.file(id).path.clone());
        assert_eq!(thumbnail, Some(root.join("SentryClips/e/thumb.png")));
        assert_eq!(
            library
                .warnings
                .iter()
                .map(|warning| (warning.kind, warning.count))
                .collect::<Vec<_>>(),
            [
                (WarningKind::EmptyMediaFile, 1),
                (WarningKind::EncryptedFile, 1)
            ]
        );
        assert_eq!(
            progress.last(),
            Some(&ScanProgress {
                processed: 39,
                total: 39
            })
        );
    }

    #[test]
    fn metadata_limits_fail_the_scan() {
        let temp = TempDir::new();
        let root = temp.directory("TeslaCam");
        temp.file(
            "TeslaCam/SentryClips/a/2026-08-21_10-00-00-front.mp4",
            b"clip",
        );
        temp.file(
            "TeslaCam/SentryClips/a/event.json",
            format!(
                r#"{{"timestamp": "2026-08-21T10:00:30", "city": "{}"}}"#,
                "x".repeat(2_049)
            )
            .as_bytes(),
        );
        let result = scan_library(&root, &mut |_| {}, &AtomicBool::new(false));
        assert_eq!(
            result.map(|_| ()),
            Err(ScanError::Limit(ScanLimitKind::MetadataString))
        );
    }

    #[test]
    fn unreadable_metadata_is_a_warning() {
        let temp = TempDir::new();
        let files = vec![FileEntry {
            name: "event.json".into(),
            relative_path: "TeslaCam/SentryClips/a/event.json".into(),
            size: 10,
            path: temp.0.join("removed/event.json"),
        }];
        let results = read_metadata_files(&files, &[FileId(0)], 4);
        assert_eq!(results.get(&FileId(0)), Some(&Ok(None)));
        assert!(read_metadata_files(&files, &[], 4).is_empty());
    }
}
