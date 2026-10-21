//! The app model: `teslacam_core::app_state::AppModel` (state) driven by its reducer (logic), plus
//! the work its effects ask for: the background folder scan, the theater session of the selected
//! event and that session's media (players and export, [`SessionMedia`]). Views read it and send it
//! actions; they never change state themselves.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use futures::StreamExt;
use gpui_kit::{AppContext as _, Context, Entity, EventEmitter, PathPromptOptions, Task};
use objc2::MainThreadMarker;
use teslacam_core::app_state::{AppAction, AppEffect, AppModel, AppState, Workspace};
use teslacam_core::camera::CameraMap;
use teslacam_core::copy;
use teslacam_core::export_state::{ExportAction, ExportEffect};
use teslacam_core::library::Event;
use teslacam_core::playback::{
    MediaCommand, PlaybackInput, PlaybackMessage, PlaybackPhase, ViewMode,
};
use teslacam_media::{EventInput, ExportError, host_now};

use crate::bridge::status_inputs;
use crate::media::{ExportUpdate, MediaReport, SessionMedia};
use crate::scan::{ScanJob, ScanMessage, outcome, spawn_scan};
use crate::session::{Session, SessionEffects};

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
    /// Bumped with every new session, so an answer of the save panel for an old one is dropped.
    session_generation: u64,
    media: Option<Entity<SessionMedia>>,
    loaded_at: Option<Instant>,
    /// Development: export here instead of asking with the save panel (`TESLACAM_EXPORT_TO`).
    export_destination: Option<PathBuf>,
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
            session_generation: 0,
            media: None,
            loaded_at: None,
            export_destination: None,
        }
    }

    /// Development: exports go to `path` without the save panel.
    pub fn set_export_destination(&mut self, path: Option<PathBuf>) {
        self.export_destination = path;
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

    /// The media of the session (absent in headless tests, which have no main thread).
    pub fn media(&self) -> Option<&Entity<SessionMedia>> {
        self.media.as_ref()
    }

    /// The playback message under the transport (hidden while `event.mp4` plays instead).
    pub fn playback_message(&self) -> Option<PlaybackMessage> {
        let playback = &self.session.as_ref()?.playback;
        if playback.view_mode() == ViewMode::Representative {
            return None;
        }
        playback.message()
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
                self.end_session(cx);
                self.start_session(&event_id, cx);
                cx.emit(StoreEvent::SessionChanged);
            }
            AppEffect::EndSession => {
                self.end_session(cx);
                cx.emit(StoreEvent::SessionChanged);
            }
        }
    }

    fn start_session(&mut self, event_id: &str, cx: &mut Context<Self>) {
        self.session_generation += 1;
        let Some(workspace) = self.workspace() else {
            return;
        };
        let library = workspace.library.clone();
        let Some(event) = library.event(event_id) else {
            return;
        };
        let Some((session, effects)) = Session::new(event, workspace.layout) else {
            return;
        };
        // AVFoundation needs the main thread; headless UI tests run the pure session only.
        self.media = MainThreadMarker::new().map(|mtm| {
            let input = EventInput::from_event(event, &library, &session.timeline);
            let representative = event
                .representative
                .map(|file| library.file(file).path.clone());
            let initial = session.playback.position();
            cx.new(|_| SessionMedia::open(input, initial, representative, host_now(), mtm))
        });
        self.session = Some(session);
        self.forward(effects.media, cx);
    }

    /// Ends the session: a running export stops silently and the players are released.
    fn end_session(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = &mut self.session {
            let effects = session.export.dispatch(ExportAction::SessionEnded);
            self.perform_export(effects, cx);
        }
        self.session = None;
        self.media = None;
    }

    /// The app is quitting: a running export is stopped and its partial output removed.
    pub fn shutdown(&mut self, cx: &mut Context<Self>) {
        if let Some(media) = &self.media {
            media.update(cx, |media, _| {
                media.abort_export_and_wait(Duration::from_secs(2));
            });
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
    ///
    /// The machine first hears where the media actually is, so skips, a restart at the end and the
    /// next start use the real position.
    pub fn playback(&mut self, input: PlaybackInput, cx: &mut Context<Self>) {
        let position = self
            .media
            .as_ref()
            .map(|media| media.read(cx).position(host_now()));
        let Some(session) = &mut self.session else {
            return;
        };
        let was_representative = session.playback.view_mode() == ViewMode::Representative;
        if let Some(position) = position {
            session.handle(PlaybackInput::Clock(position));
        }
        let effects = session.handle(input);
        self.after_playback(effects, was_representative, cx);
    }

    /// Forwards the machine's commands, opens or pauses `event.mp4` when the representative view
    /// comes or goes, and remembers a chosen layout.
    fn after_playback(
        &mut self,
        effects: SessionEffects,
        was_representative: bool,
        cx: &mut Context<Self>,
    ) {
        let representative = self
            .session
            .as_ref()
            .is_some_and(|session| session.playback.view_mode() == ViewMode::Representative);
        if let Some(media) = &self.media
            && was_representative != representative
        {
            media.update(cx, |media, _| {
                if representative {
                    media.ensure_representative();
                } else {
                    media.pause_representative(host_now());
                }
            });
        }
        self.forward(effects.media, cx);
        if let Some(layout) = effects.layout {
            self.dispatch(AppAction::LayoutChosen(layout), cx);
        } else {
            cx.notify();
        }
    }

    fn forward(&mut self, commands: Vec<MediaCommand>, cx: &mut Context<Self>) {
        if commands.is_empty() {
            return;
        }
        if let Some(media) = &self.media {
            let now = host_now();
            media.update(cx, |media, _| {
                for command in commands {
                    media.apply(command, now);
                }
            });
        }
    }

    /// Whether the frame loop must run (playing, loading, settling, exporting).
    pub fn media_active(&self, cx: &gpui_kit::App) -> bool {
        self.media
            .as_ref()
            .is_some_and(|media| media.read(cx).needs_ticks())
    }

    /// One step of the frame loop: the media applies finished work, then its report goes to the
    /// session's machines. Observers are notified only when something they show changed.
    pub fn media_tick(&mut self, now: f64, cx: &mut Context<Self>) {
        let Some(media) = self.media.clone() else {
            return;
        };
        let report = media.update(cx, |media, _| media.tick(now));
        self.apply_report(report, cx);
    }

    fn apply_report(&mut self, report: MediaReport, cx: &mut Context<Self>) {
        let Some(event) = self.selected_event().cloned() else {
            return;
        };
        let Some(session) = &mut self.session else {
            return;
        };
        let was_representative = session.playback.view_mode() == ViewMode::Representative;
        let mut inputs = Vec::new();
        let mut effects = SessionEffects::default();
        if let Some((probe, duration)) = &report.loaded {
            effects = session.media_loaded(&event, probe, *duration);
        }
        let current = CameraMap::from_fn(|camera| session.playback.status(camera));
        inputs.extend(status_inputs(&current, &report.statuses));
        if report.ended {
            inputs.push(PlaybackInput::ReachedEnd);
        }
        let mut changed = report.loaded.is_some() || !inputs.is_empty();
        for input in inputs {
            let next = session.handle(input);
            effects.media.extend(next.media);
            effects.layout = next.layout.or(effects.layout);
        }
        if report.running
            && session.playback.phase() == PlaybackPhase::Waiting
            && session.playback.intends_to_play()
        {
            effects
                .media
                .extend(session.handle(PlaybackInput::Started).media);
            changed = true;
        }
        let export_effects = match report.export {
            Some(ExportUpdate::Progressed(progress)) => {
                let before = session.export.progress().map(copy::progress_percent);
                let effects = session.export.dispatch(ExportAction::Progressed(progress));
                changed |= session.export.progress().map(copy::progress_percent) != before;
                effects
            }
            Some(ExportUpdate::Finished(path)) => {
                changed = true;
                session.export.dispatch(ExportAction::Completed(path))
            }
            // An abort was asked for; the machine already shows it.
            Some(ExportUpdate::Failed(ExportError::Aborted)) => Vec::new(),
            Some(ExportUpdate::Failed(error)) => {
                changed = true;
                session
                    .export
                    .dispatch(ExportAction::Failed(Some(error.message().to_owned())))
            }
            None => Vec::new(),
        };
        self.perform_export(export_effects, cx);
        if changed {
            self.after_playback(effects, was_representative, cx);
        }
    }

    /// Sends an action to the session's export control and performs its effects.
    pub fn export(&mut self, action: ExportAction, cx: &mut Context<Self>) {
        let Some(session) = &mut self.session else {
            return;
        };
        let effects = session.export.dispatch(action);
        self.perform_export(effects, cx);
        cx.notify();
    }

    fn perform_export(&mut self, effects: Vec<ExportEffect>, cx: &mut Context<Self>) {
        for effect in effects {
            match effect {
                ExportEffect::ChooseDestination => self.choose_destination(cx),
                ExportEffect::StartExport(path) => {
                    if let Some(media) = &self.media {
                        media.update(cx, |media, _| media.start_export(&path));
                    }
                }
                ExportEffect::CancelExport => {
                    if let Some(media) = &self.media {
                        media.update(cx, |media, _| media.cancel_export());
                    }
                }
                ExportEffect::RevealOutput(path) => cx.reveal_path(&path),
                ExportEffect::DiscardOutput(path) => {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }

    /// The save panel in `~/Movies`, named after the event (`TeslaCam-{timestamp}-vehicle.mp4`).
    fn choose_destination(&mut self, cx: &mut Context<Self>) {
        let generation = self.session_generation;
        let answer = match self.export_destination.clone() {
            // Development runs skip the panel (the answer arrives after this dispatch).
            Some(path) => {
                let (sender, receiver) = futures::channel::oneshot::channel();
                let _ = sender.send(Ok(Some(path)));
                receiver
            }
            None => {
                let Some(event) = self.selected_event() else {
                    return;
                };
                let name = copy::export_file_name(&event.timestamp);
                let home = std::env::var_os("HOME").map(PathBuf::from);
                cx.prompt_for_new_path(&export_directory(home.as_deref()), Some(&name))
            }
        };
        cx.spawn(async move |this, cx| {
            let chosen = answer.await.ok().and_then(Result::ok).flatten();
            this.update(cx, |store, cx| {
                if store.session_generation != generation {
                    return;
                }
                let action = match chosen {
                    Some(path) => ExportAction::DestinationChosen(path),
                    None => ExportAction::DestinationCancelled,
                };
                store.export(action, cx);
            })
            .ok();
        })
        .detach();
    }
}

/// Where the save panel opens: `~/Movies` when it exists, else the home folder.
pub fn export_directory(home: Option<&Path>) -> PathBuf {
    let Some(home) = home else {
        return PathBuf::from("/");
    };
    let movies = home.join("Movies");
    if movies.is_dir() {
        movies
    } else {
        home.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_save_panel_opens_in_movies_when_it_exists() {
        let home = std::env::temp_dir().join(format!("teslacam-home-{}", std::process::id()));
        std::fs::create_dir_all(&home).expect("home");
        assert_eq!(export_directory(Some(&home)), home);
        std::fs::create_dir_all(home.join("Movies")).expect("movies");
        assert_eq!(export_directory(Some(&home)), home.join("Movies"));
        assert_eq!(export_directory(None), PathBuf::from("/"));
        std::fs::remove_dir_all(&home).expect("clean up");
    }
}
