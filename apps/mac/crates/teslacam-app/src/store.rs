//! The app model: `teslacam_core::app_state::AppModel` (state) driven by its reducer (logic), plus
//! the work its effects ask for: the background folder scan and the theater session of the
//! selected event. Views read it and send it actions; they never change state themselves.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use futures::StreamExt;
use gpui_kit::{Context, EventEmitter, PathPromptOptions, Task};
use teslacam_core::app_state::{AppAction, AppEffect, AppModel, AppState, Workspace};
use teslacam_core::copy;
use teslacam_core::export_state::ExportAction;
use teslacam_core::library::Event;
use teslacam_core::playback::PlaybackInput;

use crate::scan::{ScanJob, ScanMessage, outcome, spawn_scan};
use crate::session::Session;

/// What changed, for views that keep their own UI state (an open cluster, an open popover).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreEvent {
    /// A newly scanned library replaced the previous one.
    LibraryLoaded,
    /// Another event was selected (or the selection was cleared).
    SessionChanged,
}

/// The app's state and the work around it.
pub struct AppStore {
    model: AppModel,
    root: Option<PathBuf>,
    scan: Option<ScanJob>,
    scan_task: Option<Task<()>>,
    scan_delay: Duration,
    session: Option<Session>,
    loaded_at: Option<Instant>,
}

impl EventEmitter<StoreEvent> for AppStore {}

impl AppStore {
    /// The welcome screen. `scan_delay` slows scans down for development (see `dev`).
    pub fn new(scan_delay: Duration) -> AppStore {
        AppStore {
            model: AppModel::new(),
            root: None,
            scan: None,
            scan_task: None,
            scan_delay,
            session: None,
            loaded_at: None,
        }
    }

    /// The app state.
    pub fn state(&self) -> &AppState {
        self.model.state()
    }

    /// The loaded workspace, if any.
    pub fn workspace(&self) -> Option<&Workspace> {
        match self.model.state() {
            AppState::Ready(workspace) => Some(workspace),
            _ => None,
        }
    }

    /// The selected event.
    pub fn selected_event(&self) -> Option<&Event> {
        self.workspace()?.selected_event()
    }

    /// The theater session of the selected event.
    pub fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    /// When the current library finished loading (the start of the window reveal).
    pub fn loaded_at(&self) -> Option<Instant> {
        self.loaded_at
    }

    /// Applies `action` and performs its effects.
    pub fn dispatch(&mut self, action: AppAction, cx: &mut Context<Self>) {
        let loading = matches!(action, AppAction::LibraryReady { .. });
        let effects = self.model.dispatch(action);
        if loading && matches!(self.model.state(), AppState::Ready(_)) {
            self.loaded_at = Some(Instant::now());
            cx.emit(StoreEvent::LibraryLoaded);
        }
        for effect in effects {
            self.perform(effect, cx);
        }
        cx.notify();
    }

    fn perform(&mut self, effect: AppEffect, cx: &mut Context<Self>) {
        match effect {
            AppEffect::StartScan { generation } => self.start_scan(generation, cx),
            AppEffect::CancelScan { generation } => {
                if let Some(job) = self.scan.take_if(|job| job.generation == generation) {
                    job.cancel();
                    self.scan_task = None;
                }
            }
            AppEffect::StartSession { event_id } => {
                self.session = self.workspace().and_then(|workspace| {
                    let event = workspace.library.event(&event_id)?;
                    Session::new(event, workspace.layout).map(|(session, _media)| session)
                });
                cx.emit(StoreEvent::SessionChanged);
            }
            AppEffect::EndSession => {
                self.session = None;
                cx.emit(StoreEvent::SessionChanged);
            }
        }
    }

    /// Shows the open panel (directories only) and scans the chosen folder.
    pub fn choose_folder(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(copy::WELCOME_ACTION.into()),
        });
        cx.spawn(async move |this, cx| {
            let picked = paths
                .await
                .ok()
                .and_then(Result::ok)
                .flatten()
                .and_then(|mut paths| paths.pop());
            this.update(cx, |store, cx| match picked {
                Some(path) => store.open_folder(path, cx),
                None => store.dispatch(AppAction::PickerCancelled, cx),
            })
            .ok();
        })
        .detach();
    }

    /// Scans `path` (the open panel's choice).
    pub fn open_folder(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.root = Some(path);
        self.dispatch(AppAction::ScanStarted, cx);
    }

    /// Cancels the running scan.
    pub fn cancel_scan(&mut self, cx: &mut Context<Self>) {
        self.dispatch(AppAction::ScanCancelled, cx);
    }

    fn start_scan(&mut self, generation: u64, cx: &mut Context<Self>) {
        let Some(root) = self.root.clone() else {
            return;
        };
        let (job, mut receiver) = spawn_scan(root, generation, self.scan_delay);
        self.scan = Some(job);
        self.scan_task = Some(cx.spawn(async move |this, cx| {
            while let Some(first) = receiver.next().await {
                // Coalesce progress reports that piled up while the main thread was busy.
                let mut messages = vec![first];
                while let Ok(message) = receiver.try_recv() {
                    messages.push(message);
                }
                let latest_progress = messages
                    .iter()
                    .rposition(|message| matches!(message, ScanMessage::Progress(_)));
                let keep = messages.into_iter().enumerate().filter(|(index, message)| {
                    !matches!(message, ScanMessage::Progress(_)) || Some(*index) == latest_progress
                });
                for (_, message) in keep {
                    let updated = this.update(cx, |store, cx| {
                        store.on_scan_message(generation, message, cx);
                    });
                    if updated.is_err() {
                        return;
                    }
                }
            }
        }));
    }

    fn on_scan_message(&mut self, generation: u64, message: ScanMessage, cx: &mut Context<Self>) {
        match message {
            ScanMessage::Progress(progress) => self.dispatch(
                AppAction::ScanProgressed {
                    generation,
                    progress,
                },
                cx,
            ),
            ScanMessage::Finished(result) => {
                if self
                    .scan
                    .as_ref()
                    .is_some_and(|job| job.generation == generation)
                {
                    self.scan = None;
                }
                if let Some(action) = outcome(generation, result) {
                    self.dispatch(action, cx);
                }
            }
        }
    }

    /// Sends a playback input to the session; a chosen layout is remembered by the workspace.
    pub fn playback(&mut self, input: PlaybackInput, cx: &mut Context<Self>) {
        let Some(session) = &mut self.session else {
            return;
        };
        let effects = session.handle(input);
        // Stage 2: forward `effects.media` to the media layer.
        if let Some(layout) = effects.layout {
            self.dispatch(AppAction::LayoutChosen(layout), cx);
        } else {
            cx.notify();
        }
    }

    /// Sends an action to the session's export control.
    pub fn export(&mut self, action: ExportAction, cx: &mut Context<Self>) {
        let Some(session) = &mut self.session else {
            return;
        };
        // Stage 2: perform the returned effects (start, cancel, reveal the output).
        let _effects = session.export.dispatch(action);
        cx.notify();
    }
}
