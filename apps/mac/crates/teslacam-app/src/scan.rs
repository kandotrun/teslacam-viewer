//! The background folder scan: a worker thread runs `teslacam_core::scan::scan_library` and streams
//! its progress and result to the main thread, which turns them into app actions.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use futures::channel::mpsc::{UnboundedReceiver, unbounded};
use teslacam_core::app_state::AppAction;
use teslacam_core::library::ScanProgress;
use teslacam_core::scan::{ScanError, ScannedFolder, scan_library};

/// What the worker reports.
#[derive(Debug)]
pub enum ScanMessage {
    /// Entries or files processed so far.
    Progress(ScanProgress),
    /// The scan ended.
    Finished(Result<ScannedFolder, ScanError>),
}

/// A running scan.
pub struct ScanJob {
    /// The generation the app state machine gave this scan.
    pub generation: u64,
    /// Set to stop the worker.
    pub cancel: Arc<AtomicBool>,
}

impl ScanJob {
    /// Asks the worker to stop; it reports nothing more that matters.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

/// Starts scanning `root` on a worker thread. `delay` slows each progress report down (development
/// only, see `dev`).
pub fn spawn_scan(
    root: PathBuf,
    generation: u64,
    delay: Duration,
) -> (ScanJob, UnboundedReceiver<ScanMessage>) {
    let (sender, receiver) = unbounded();
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    thread::Builder::new()
        .name("teslacam-scan".into())
        .spawn(move || {
            let progress_sender = sender.clone();
            let mut progress = move |progress: ScanProgress| {
                let _ = progress_sender.unbounded_send(ScanMessage::Progress(progress));
                if !delay.is_zero() {
                    thread::sleep(delay);
                }
            };
            let result = scan_library(&root, &mut progress, &worker_cancel);
            let _ = sender.unbounded_send(ScanMessage::Finished(result));
        })
        .expect("spawn the scan thread");
    (ScanJob { generation, cancel }, receiver)
}

/// The app action for a finished scan of `generation` (spec §1.3 result mapping); `None` for a
/// cancelled scan, which the state machine already left.
pub fn outcome(generation: u64, result: Result<ScannedFolder, ScanError>) -> Option<AppAction> {
    match result {
        Ok(folder) if folder.library.files.is_empty() => {
            Some(AppAction::ScanInvalid { generation })
        }
        Ok(folder) => Some(AppAction::LibraryReady {
            generation,
            folder_name: folder.folder_name,
            library: Arc::new(folder.library),
        }),
        Err(ScanError::Cancelled) => None,
        Err(ScanError::Denied) => Some(AppAction::ScanDenied { generation }),
        Err(ScanError::Limit(_) | ScanError::Io(_)) => Some(AppAction::ScanFailed { generation }),
    }
}

#[cfg(test)]
mod tests {
    use std::io::ErrorKind;

    use futures::StreamExt;
    use teslacam_core::library::{FileEntry, Library};
    use teslacam_core::limits::ScanLimitKind;

    use super::*;

    fn folder(files: Vec<FileEntry>) -> ScannedFolder {
        ScannedFolder {
            folder_name: "TeslaCam".into(),
            library: Library {
                files,
                ..Library::default()
            },
        }
    }

    #[test]
    fn results_map_to_actions() {
        assert_eq!(
            outcome(3, Ok(folder(Vec::new()))),
            Some(AppAction::ScanInvalid { generation: 3 })
        );
        let ready = outcome(
            4,
            Ok(folder(vec![FileEntry::new(
                "notes.txt",
                "TeslaCam/notes.txt",
                1,
            )])),
        );
        assert!(matches!(
            ready,
            Some(AppAction::LibraryReady { generation: 4, ref folder_name, .. }) if folder_name == "TeslaCam"
        ));
        assert_eq!(outcome(5, Err(ScanError::Cancelled)), None);
        assert_eq!(
            outcome(6, Err(ScanError::Denied)),
            Some(AppAction::ScanDenied { generation: 6 })
        );
        assert_eq!(
            outcome(7, Err(ScanError::Limit(ScanLimitKind::EntryCount))),
            Some(AppAction::ScanFailed { generation: 7 })
        );
        assert_eq!(
            outcome(8, Err(ScanError::Io(ErrorKind::NotFound))),
            Some(AppAction::ScanFailed { generation: 8 })
        );
    }

    #[test]
    fn a_worker_reports_progress_and_the_result() {
        let root = std::env::temp_dir().join(format!("teslacam-scan-test-{}", std::process::id()));
        let clips = root.join("SavedClips/event");
        std::fs::create_dir_all(&clips).expect("create fixture");
        std::fs::write(clips.join("2026-08-20_20-40-00-front.mp4"), b"x").expect("write clip");
        let (job, receiver) = spawn_scan(root.clone(), 1, Duration::ZERO);
        let messages = futures::executor::block_on(receiver.collect::<Vec<_>>());
        std::fs::remove_dir_all(&root).ok();
        assert_eq!(job.generation, 1);
        assert!(matches!(messages.first(), Some(ScanMessage::Progress(_))));
        let Some(ScanMessage::Finished(Ok(folder))) = messages.last() else {
            panic!("no result: {messages:?}");
        };
        assert_eq!(folder.library.events.len(), 1);
    }
}
