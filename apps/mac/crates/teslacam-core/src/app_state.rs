//! The app state machine: welcome, scanning, errors and the workspace (spec §1.21, §1.19,
//! `DESIGN.md` day navigation).
//!
//! The reducer is pure: [`AppModel::dispatch`] returns [`AppEffect`]s that the app performs (start or
//! cancel a background scan, start or end a theater session). Scan results carry the generation of
//! the scan that produced them; results of an older scan are ignored.
//!
//! Mac decisions on top of the web reducer:
//!
//! - Dismissing the open panel while a library is loaded keeps it (the web discards it, spec A-1),
//!   and cancelling a running re-scan restores the previous workspace.
//! - A freshly loaded library selects its newest event, so the stage fills with video right away
//!   (`DESIGN.md` §7); the web starts without a selection.
//! - Choosing a day (date picker or `前の記録日` / `次の記録日`) filters to that day and selects its
//!   newest event. Unknown event ids are ignored.
//!
//! Shared with the web's garage floor (`presentation/library-view.ts`): `過去の記録` / `新しい記録`
//! step through the rail's events (category and query apply, the day does not), and selecting an
//! event on another day moves an active date filter to that day.

use std::sync::Arc;

use crate::copy::FolderError;
use crate::day_timeline::{
    TimelineDay, adjacent_timeline_days, build_timeline_days, select_timeline_date,
};
use crate::filters::{
    CategoryFilter, EventFilters, event_date, event_matches_filters, ordered_matching_events,
};
use crate::library::{Event, Library, ScanProgress};
use crate::playback::StageLayout;

/// The loaded folder.
#[derive(Debug, Clone, PartialEq)]
pub struct Workspace {
    /// The folder's display name.
    pub folder_name: String,
    /// The library.
    pub library: Arc<Library>,
    /// Current filters.
    pub filters: EventFilters,
    /// The selected event (owner of the theater session).
    pub selected_event_id: Option<String>,
    /// The stage layout, kept across events.
    pub layout: StageLayout,
}

/// Previous or next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Step {
    /// Older event / earlier day.
    Previous,
    /// Newer event / later day.
    Next,
}

impl Workspace {
    /// The event list: matching events, newest first.
    pub fn matching_events(&self) -> Vec<&Event> {
        ordered_matching_events(&self.library.events, &self.filters)
    }

    /// The day rail's input: category and query apply, the date filter does not.
    pub fn timeline_events(&self) -> Vec<&Event> {
        let filters = EventFilters {
            date_key: None,
            ..self.filters.clone()
        };
        ordered_matching_events(&self.library.events, &filters)
    }

    /// The selected event.
    pub fn selected_event(&self) -> Option<&Event> {
        self.library.event(self.selected_event_id.as_deref()?)
    }

    /// The day the rail shows.
    pub fn timeline_date(&self) -> Option<String> {
        select_timeline_date(
            &self.timeline_events(),
            self.selected_event(),
            self.filters.date_key.as_deref(),
        )
    }

    /// Recorded days of the rail's events.
    pub fn timeline_days(&self) -> Vec<TimelineDay> {
        build_timeline_days(&self.timeline_events())
    }

    /// The recorded day before or after the shown one.
    pub fn adjacent_day(&self, step: Step) -> Option<TimelineDay> {
        let days = self.timeline_days();
        let date = self.timeline_date();
        let adjacency = adjacent_timeline_days(&days, date.as_deref());
        match step {
            Step::Previous => adjacency.previous,
            Step::Next => adjacency.next,
        }
        .cloned()
    }

    /// The event `step` away from the selection among [`Self::timeline_events`].
    pub fn adjacent_event(&self, step: Step) -> Option<&Event> {
        let events = self.timeline_events();
        let index = events
            .iter()
            .position(|event| Some(&event.id) == self.selected_event_id.as_ref())?;
        let target = match step {
            Step::Previous => index.checked_add(1)?,
            Step::Next => index.checked_sub(1)?,
        };
        events.get(target).copied()
    }
}

/// Where the app is.
#[derive(Debug, Clone, PartialEq)]
pub enum AppState {
    /// Welcome.
    Idle,
    /// A scan runs.
    Scanning {
        /// The scan's generation.
        generation: u64,
        /// Latest progress.
        progress: ScanProgress,
        /// The workspace to return to when the scan is cancelled.
        previous: Option<Box<Workspace>>,
    },
    /// Welcome with an error.
    Error {
        /// What went wrong.
        reason: FolderError,
    },
    /// A library is loaded.
    Ready(Workspace),
}

/// Inputs from the UI and the background scan.
#[derive(Debug, Clone, PartialEq)]
pub enum AppAction {
    /// A folder was chosen in the open panel.
    ScanStarted,
    /// The open panel was dismissed.
    PickerCancelled,
    /// The user cancelled the running scan.
    ScanCancelled,
    /// Progress of scan `generation`.
    ScanProgressed {
        /// The scan.
        generation: u64,
        /// Its progress.
        progress: ScanProgress,
    },
    /// Scan `generation` could not read the folder.
    ScanDenied {
        /// The scan.
        generation: u64,
    },
    /// Scan `generation` found no files.
    ScanInvalid {
        /// The scan.
        generation: u64,
    },
    /// Scan `generation` failed (I/O error or scan limit).
    ScanFailed {
        /// The scan.
        generation: u64,
    },
    /// Scan `generation` built a library.
    LibraryReady {
        /// The scan.
        generation: u64,
        /// The folder's display name.
        folder_name: String,
        /// The library.
        library: Arc<Library>,
    },
    /// The category segmented control.
    CategoryFiltered(CategoryFilter),
    /// Sets or clears the date filter without changing the selection rules.
    DateFiltered(Option<String>),
    /// The date picker: filter to the day and play its newest event.
    DayChosen(String),
    /// `前の記録日` / `次の記録日`.
    DayStepped(Step),
    /// The search field.
    QueryFiltered(String),
    /// A row, marker or cluster member.
    EventSelected(String),
    /// `過去の記録` / `新しい記録`, `[` / `]`.
    EventStepped(Step),
    /// The view segmented control chose a layout.
    LayoutChosen(StageLayout),
}

/// Work for the app shell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEffect {
    /// Start scanning the chosen folder; report results with this generation.
    StartScan {
        /// The new scan's generation.
        generation: u64,
    },
    /// Stop the scan of this generation.
    CancelScan {
        /// The scan to stop.
        generation: u64,
    },
    /// Create a theater session for the event (replacing any current one).
    StartSession {
        /// The event.
        event_id: String,
    },
    /// Drop the theater session without a replacement.
    EndSession,
}

/// The app state plus the scan generation counter.
#[derive(Debug, Clone, PartialEq)]
pub struct AppModel {
    state: AppState,
    generation: u64,
}

impl Default for AppModel {
    fn default() -> AppModel {
        AppModel::new()
    }
}

impl AppModel {
    /// The welcome screen.
    pub fn new() -> AppModel {
        AppModel {
            state: AppState::Idle,
            generation: 0,
        }
    }

    /// The current state.
    pub fn state(&self) -> &AppState {
        &self.state
    }

    /// The generation of the latest scan.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Applies `action` and returns the effects to perform.
    pub fn dispatch(&mut self, action: AppAction) -> Vec<AppEffect> {
        match action {
            AppAction::ScanStarted => self.start_scan(),
            AppAction::PickerCancelled => {
                if matches!(self.state, AppState::Error { .. }) {
                    self.state = AppState::Idle;
                }
                Vec::new()
            }
            AppAction::ScanCancelled => self.cancel_scan(),
            AppAction::ScanProgressed {
                generation,
                progress,
            } => {
                if let AppState::Scanning {
                    generation: current,
                    progress: latest,
                    ..
                } = &mut self.state
                    && *current == generation
                {
                    *latest = progress;
                }
                Vec::new()
            }
            AppAction::ScanDenied { generation } => self.fail_scan(generation, FolderError::Denied),
            AppAction::ScanInvalid { generation } => {
                self.fail_scan(generation, FolderError::Invalid)
            }
            AppAction::ScanFailed { generation } => self.fail_scan(generation, FolderError::Failed),
            AppAction::LibraryReady {
                generation,
                folder_name,
                library,
            } => self.open_library(generation, folder_name, library),
            AppAction::CategoryFiltered(category) => self.with_workspace(|workspace| {
                let filters = EventFilters {
                    category,
                    ..workspace.filters.clone()
                };
                set_filters(workspace, filters)
            }),
            AppAction::DateFiltered(date_key) => self.with_workspace(|workspace| {
                let filters = EventFilters {
                    date_key,
                    ..workspace.filters.clone()
                };
                set_filters(workspace, filters)
            }),
            AppAction::QueryFiltered(query) => self.with_workspace(|workspace| {
                let filters = EventFilters {
                    query,
                    ..workspace.filters.clone()
                };
                set_filters(workspace, filters)
            }),
            AppAction::DayChosen(date_key) => {
                self.with_workspace(|workspace| choose_day(workspace, date_key))
            }
            AppAction::DayStepped(step) => {
                self.with_workspace(|workspace| match workspace.adjacent_day(step) {
                    Some(day) => choose_day(workspace, day.date_key),
                    None => Vec::new(),
                })
            }
            AppAction::EventSelected(event_id) => {
                self.with_workspace(|workspace| select(workspace, event_id))
            }
            AppAction::EventStepped(step) => {
                self.with_workspace(|workspace| match workspace.adjacent_event(step) {
                    Some(event) => {
                        let event_id = event.id.clone();
                        select(workspace, event_id)
                    }
                    None => Vec::new(),
                })
            }
            AppAction::LayoutChosen(layout) => self.with_workspace(|workspace| {
                workspace.layout = layout;
                Vec::new()
            }),
        }
    }

    fn with_workspace(
        &mut self,
        update: impl FnOnce(&mut Workspace) -> Vec<AppEffect>,
    ) -> Vec<AppEffect> {
        match &mut self.state {
            AppState::Ready(workspace) => update(workspace),
            _ => Vec::new(),
        }
    }

    fn is_current_scan(&self, generation: u64) -> bool {
        matches!(self.state, AppState::Scanning { generation: current, .. } if current == generation)
    }

    fn start_scan(&mut self) -> Vec<AppEffect> {
        let mut effects = Vec::new();
        let previous = match std::mem::replace(&mut self.state, AppState::Idle) {
            AppState::Ready(workspace) => {
                if workspace.selected_event_id.is_some() {
                    effects.push(AppEffect::EndSession);
                }
                Some(Box::new(workspace))
            }
            AppState::Scanning {
                generation,
                previous,
                ..
            } => {
                effects.push(AppEffect::CancelScan { generation });
                previous
            }
            AppState::Idle | AppState::Error { .. } => None,
        };
        self.generation += 1;
        self.state = AppState::Scanning {
            generation: self.generation,
            progress: ScanProgress::default(),
            previous,
        };
        effects.push(AppEffect::StartScan {
            generation: self.generation,
        });
        effects
    }

    fn cancel_scan(&mut self) -> Vec<AppEffect> {
        let AppState::Scanning {
            generation,
            previous,
            ..
        } = std::mem::replace(&mut self.state, AppState::Idle)
        else {
            return Vec::new();
        };
        let mut effects = vec![AppEffect::CancelScan { generation }];
        if let Some(workspace) = previous {
            if let Some(event_id) = &workspace.selected_event_id {
                effects.push(AppEffect::StartSession {
                    event_id: event_id.clone(),
                });
            }
            self.state = AppState::Ready(*workspace);
        }
        effects
    }

    fn fail_scan(&mut self, generation: u64, reason: FolderError) -> Vec<AppEffect> {
        if self.is_current_scan(generation) {
            self.state = AppState::Error { reason };
        }
        Vec::new()
    }

    fn open_library(
        &mut self,
        generation: u64,
        folder_name: String,
        library: Arc<Library>,
    ) -> Vec<AppEffect> {
        if !self.is_current_scan(generation) {
            return Vec::new();
        }
        let Some(newest) = library.events.first().map(|event| event.id.clone()) else {
            self.state = AppState::Error {
                reason: FolderError::Invalid,
            };
            return Vec::new();
        };
        self.state = AppState::Ready(Workspace {
            folder_name,
            library,
            filters: EventFilters::default(),
            selected_event_id: Some(newest.clone()),
            layout: StageLayout::default(),
        });
        vec![AppEffect::StartSession { event_id: newest }]
    }
}

fn set_filters(workspace: &mut Workspace, filters: EventFilters) -> Vec<AppEffect> {
    workspace.filters = filters;
    let still_matches = workspace
        .selected_event()
        .is_some_and(|event| event_matches_filters(event, &workspace.filters));
    if workspace.selected_event_id.is_some() && !still_matches {
        workspace.selected_event_id = None;
        vec![AppEffect::EndSession]
    } else {
        Vec::new()
    }
}

fn select(workspace: &mut Workspace, event_id: String) -> Vec<AppEffect> {
    if workspace.selected_event_id.as_ref() == Some(&event_id) {
        return Vec::new();
    }
    let Some(event) = workspace.library.event(&event_id) else {
        return Vec::new();
    };
    let event_date = event_date(&event.timestamp);
    if let Some(date_key) = &mut workspace.filters.date_key
        && *date_key != event_date
    {
        *date_key = event_date;
    }
    workspace.selected_event_id = Some(event_id.clone());
    vec![AppEffect::StartSession { event_id }]
}

fn choose_day(workspace: &mut Workspace, date_key: String) -> Vec<AppEffect> {
    let newest = workspace
        .timeline_days()
        .into_iter()
        .find(|day| day.date_key == date_key)
        .and_then(|day| day.newest_event_id().map(str::to_owned));
    let mut effects = set_filters(
        workspace,
        EventFilters {
            date_key: Some(date_key),
            ..workspace.filters.clone()
        },
    );
    if let Some(event_id) = newest {
        let selection = select(workspace, event_id);
        if !selection.is_empty() {
            effects = selection;
        }
    }
    effects
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::ClipCategory;
    use crate::library::{FileEntry, build_library};

    fn library(paths: &[&str]) -> Arc<Library> {
        let files = paths
            .iter()
            .map(|path| FileEntry::new(path.rsplit('/').next().expect("name"), *path, 10))
            .collect();
        Arc::new(build_library(files, |_| {}, |_, _| Ok(None)).expect("library"))
    }

    fn history() -> Arc<Library> {
        library(&[
            "TeslaCam/RecentClips/2026-08-20_20-45-00-front.mp4",
            "TeslaCam/SavedClips/event-saved/2026-08-20_20-40-00-front.mp4",
            "TeslaCam/SentryClips/event-sentry/2026-08-20_20-35-00-front.mp4",
            "TeslaCam/SavedClips/event-saved-previous/2026-08-19_19-30-00-front.mp4",
            "TeslaCam/SavedClips/morning/2026-08-19_08-00-00-front.mp4",
        ])
    }

    const RECENT: &str = "recent-2026-08-20_20-45-00-1";
    const SAVED: &str = "saved-2026-08-20_20-40-00-1";
    const SENTRY: &str = "sentry-2026-08-20_20-35-00-1";
    const PREVIOUS: &str = "saved-2026-08-19_19-30-00-2";
    const MORNING: &str = "saved-2026-08-19_08-00-00-3";

    fn ready(library: Arc<Library>) -> AppModel {
        let mut model = AppModel::new();
        model.dispatch(AppAction::ScanStarted);
        let generation = model.generation();
        model.dispatch(AppAction::LibraryReady {
            generation,
            folder_name: "TeslaCam".into(),
            library,
        });
        model
    }

    fn workspace(model: &AppModel) -> &Workspace {
        match model.state() {
            AppState::Ready(workspace) => workspace,
            other => panic!("not ready: {other:?}"),
        }
    }

    fn selected(model: &AppModel) -> Option<&str> {
        workspace(model).selected_event_id.as_deref()
    }

    #[test]
    fn starts_on_the_welcome_screen() {
        let model = AppModel::default();
        assert_eq!(model.state(), &AppState::Idle);
        assert_eq!(model.generation(), 0);
    }

    #[test]
    fn a_scan_reports_progress_and_opens_the_newest_event() {
        let mut model = AppModel::new();
        assert_eq!(
            model.dispatch(AppAction::ScanStarted),
            vec![AppEffect::StartScan { generation: 1 }]
        );
        let progress = ScanProgress {
            processed: 250,
            total: 250,
        };
        model.dispatch(AppAction::ScanProgressed {
            generation: 1,
            progress,
        });
        assert_eq!(
            model.state(),
            &AppState::Scanning {
                generation: 1,
                progress,
                previous: None
            }
        );
        let effects = model.dispatch(AppAction::LibraryReady {
            generation: 1,
            folder_name: "TeslaCam".into(),
            library: history(),
        });
        assert_eq!(
            effects,
            vec![AppEffect::StartSession {
                event_id: RECENT.into()
            }]
        );
        let workspace = workspace(&model);
        assert_eq!(workspace.folder_name, "TeslaCam");
        assert_eq!(workspace.filters, EventFilters::default());
        assert_eq!(workspace.layout, StageLayout::Vehicle);
        assert_eq!(selected(&model), Some(RECENT));
    }

    #[test]
    fn results_of_an_older_scan_are_ignored() {
        let mut model = AppModel::new();
        model.dispatch(AppAction::ScanStarted);
        assert_eq!(
            model.dispatch(AppAction::ScanStarted),
            vec![
                AppEffect::CancelScan { generation: 1 },
                AppEffect::StartScan { generation: 2 }
            ]
        );
        model.dispatch(AppAction::ScanProgressed {
            generation: 1,
            progress: ScanProgress {
                processed: 9,
                total: 9,
            },
        });
        assert!(
            model
                .dispatch(AppAction::LibraryReady {
                    generation: 1,
                    folder_name: "old".into(),
                    library: history()
                })
                .is_empty()
        );
        assert!(
            model
                .dispatch(AppAction::ScanFailed { generation: 1 })
                .is_empty()
        );
        assert_eq!(
            model.state(),
            &AppState::Scanning {
                generation: 2,
                progress: ScanProgress::default(),
                previous: None
            }
        );
        model.dispatch(AppAction::LibraryReady {
            generation: 2,
            folder_name: "new".into(),
            library: history(),
        });
        assert_eq!(workspace(&model).folder_name, "new");
        model.dispatch(AppAction::ScanProgressed {
            generation: 2,
            progress: ScanProgress {
                processed: 1,
                total: 1,
            },
        });
        assert!(matches!(model.state(), AppState::Ready(_)));
    }

    #[test]
    fn scan_failures_show_errors() {
        for (action, reason) in [
            (AppAction::ScanDenied { generation: 1 }, FolderError::Denied),
            (
                AppAction::ScanInvalid { generation: 1 },
                FolderError::Invalid,
            ),
            (AppAction::ScanFailed { generation: 1 }, FolderError::Failed),
            (
                AppAction::LibraryReady {
                    generation: 1,
                    folder_name: "x".into(),
                    library: library(&["SyntheticFolder/notes.txt"]),
                },
                FolderError::Invalid,
            ),
        ] {
            let mut model = AppModel::new();
            model.dispatch(AppAction::ScanStarted);
            assert!(model.dispatch(action).is_empty());
            assert_eq!(model.state(), &AppState::Error { reason });
        }
    }

    #[test]
    fn dismissing_the_panel_keeps_the_loaded_library() {
        let mut model = ready(history());
        let before = model.clone();
        assert!(model.dispatch(AppAction::PickerCancelled).is_empty());
        assert_eq!(model, before);
        let mut error = AppModel::new();
        error.dispatch(AppAction::ScanStarted);
        error.dispatch(AppAction::ScanFailed { generation: 1 });
        error.dispatch(AppAction::PickerCancelled);
        assert_eq!(error.state(), &AppState::Idle);
    }

    #[test]
    fn cancelling_a_rescan_restores_the_workspace() {
        let mut model = ready(history());
        model.dispatch(AppAction::LayoutChosen(StageLayout::Grid));
        assert_eq!(
            model.dispatch(AppAction::ScanStarted),
            vec![
                AppEffect::EndSession,
                AppEffect::StartScan { generation: 2 }
            ]
        );
        assert_eq!(
            model.dispatch(AppAction::ScanCancelled),
            vec![
                AppEffect::CancelScan { generation: 2 },
                AppEffect::StartSession {
                    event_id: RECENT.into()
                }
            ]
        );
        assert_eq!(selected(&model), Some(RECENT));
        assert_eq!(workspace(&model).layout, StageLayout::Grid);
        let mut fresh = AppModel::new();
        fresh.dispatch(AppAction::ScanStarted);
        assert_eq!(
            fresh.dispatch(AppAction::ScanCancelled),
            vec![AppEffect::CancelScan { generation: 1 }]
        );
        assert_eq!(fresh.state(), &AppState::Idle);
    }

    #[test]
    fn a_new_library_resets_filters_and_layout() {
        let mut model = ready(history());
        model.dispatch(AppAction::LayoutChosen(StageLayout::Grid));
        model.dispatch(AppAction::QueryFiltered("saved".into()));
        model.dispatch(AppAction::ScanStarted);
        model.dispatch(AppAction::LibraryReady {
            generation: 2,
            folder_name: "TeslaCam".into(),
            library: history(),
        });
        assert_eq!(workspace(&model).layout, StageLayout::Vehicle);
        assert_eq!(workspace(&model).filters, EventFilters::default());
    }

    #[test]
    fn filters_clear_a_selection_that_no_longer_matches() {
        let mut model = ready(history());
        model.dispatch(AppAction::EventSelected(SENTRY.into()));
        assert!(
            model
                .dispatch(AppAction::CategoryFiltered(CategoryFilter::Only(
                    ClipCategory::Sentry
                )))
                .is_empty()
        );
        assert!(
            model
                .dispatch(AppAction::QueryFiltered(" 20:35 ".into()))
                .is_empty()
        );
        assert!(
            model
                .dispatch(AppAction::DateFiltered(Some("2026-08-20".into())))
                .is_empty()
        );
        assert_eq!(selected(&model), Some(SENTRY));
        assert_eq!(
            model.dispatch(AppAction::CategoryFiltered(CategoryFilter::Only(
                ClipCategory::Saved
            ))),
            vec![AppEffect::EndSession]
        );
        assert_eq!(selected(&model), None);
        assert!(
            model
                .dispatch(AppAction::QueryFiltered("nothing".into()))
                .is_empty()
        );
    }

    #[test]
    fn selecting_events() {
        let mut model = ready(history());
        assert!(
            model
                .dispatch(AppAction::EventSelected(RECENT.into()))
                .is_empty()
        );
        assert_eq!(
            model.dispatch(AppAction::EventSelected(SAVED.into())),
            vec![AppEffect::StartSession {
                event_id: SAVED.into()
            }]
        );
        assert!(
            model
                .dispatch(AppAction::EventSelected("missing".into()))
                .is_empty()
        );
        assert_eq!(selected(&model), Some(SAVED));
        assert!(
            AppModel::new()
                .dispatch(AppAction::EventSelected(SAVED.into()))
                .is_empty()
        );
    }

    #[test]
    fn stepping_through_events() {
        let mut model = ready(history());
        assert_eq!(
            model.dispatch(AppAction::EventStepped(Step::Previous)),
            vec![AppEffect::StartSession {
                event_id: SAVED.into()
            }]
        );
        assert_eq!(
            model.dispatch(AppAction::EventStepped(Step::Previous)),
            vec![AppEffect::StartSession {
                event_id: SENTRY.into()
            }]
        );
        assert_eq!(
            model.dispatch(AppAction::EventStepped(Step::Next)),
            vec![AppEffect::StartSession {
                event_id: SAVED.into()
            }]
        );
        model.dispatch(AppAction::EventSelected(MORNING.into()));
        assert!(
            model
                .dispatch(AppAction::EventStepped(Step::Previous))
                .is_empty()
        );
        model.dispatch(AppAction::EventSelected(RECENT.into()));
        assert!(
            model
                .dispatch(AppAction::EventStepped(Step::Next))
                .is_empty()
        );
        assert_eq!(
            workspace(&model)
                .adjacent_event(Step::Previous)
                .map(|event| event.id.as_str()),
            Some(SAVED)
        );
    }

    #[test]
    fn stepping_across_days_moves_the_date_filter() {
        let mut model = ready(history());
        model.dispatch(AppAction::DayStepped(Step::Previous));
        assert_eq!(selected(&model), Some(PREVIOUS));
        assert_eq!(
            model.dispatch(AppAction::EventStepped(Step::Next)),
            vec![AppEffect::StartSession {
                event_id: SENTRY.into()
            }]
        );
        assert_eq!(
            workspace(&model).filters.date_key.as_deref(),
            Some("2026-08-20")
        );
        model.dispatch(AppAction::EventSelected(MORNING.into()));
        assert_eq!(
            workspace(&model).filters.date_key.as_deref(),
            Some("2026-08-19")
        );
        model.dispatch(AppAction::DateFiltered(None));
        model.dispatch(AppAction::EventSelected(RECENT.into()));
        assert_eq!(workspace(&model).filters.date_key, None);
    }

    #[test]
    fn day_navigation_selects_the_newest_event_of_the_day() {
        let mut model = ready(history());
        assert_eq!(workspace(&model).adjacent_day(Step::Next), None);
        assert_eq!(
            model.dispatch(AppAction::DayStepped(Step::Previous)),
            vec![AppEffect::StartSession {
                event_id: PREVIOUS.into()
            }]
        );
        assert_eq!(
            workspace(&model).filters.date_key.as_deref(),
            Some("2026-08-19")
        );
        assert_eq!(workspace(&model).matching_events().len(), 2);
        assert!(
            model
                .dispatch(AppAction::DayStepped(Step::Previous))
                .is_empty()
        );
        assert_eq!(
            model.dispatch(AppAction::DayStepped(Step::Next)),
            vec![AppEffect::StartSession {
                event_id: RECENT.into()
            }]
        );
        assert_eq!(
            workspace(&model).filters.date_key.as_deref(),
            Some("2026-08-20")
        );
    }

    #[test]
    fn choosing_a_day_without_events_clears_the_selection() {
        let mut model = ready(history());
        assert_eq!(
            model.dispatch(AppAction::DayChosen("2026-08-19".into())),
            vec![AppEffect::StartSession {
                event_id: PREVIOUS.into()
            }]
        );
        assert_eq!(
            model.dispatch(AppAction::DayChosen("2026-08-01".into())),
            vec![AppEffect::EndSession]
        );
        assert_eq!(selected(&model), None);
        assert!(workspace(&model).matching_events().is_empty());
        model.dispatch(AppAction::DateFiltered(None));
        assert_eq!(workspace(&model).matching_events().len(), 5);
    }

    #[test]
    fn the_timeline_ignores_the_date_filter() {
        let mut model = ready(history());
        model.dispatch(AppAction::CategoryFiltered(CategoryFilter::Only(
            ClipCategory::Saved,
        )));
        model.dispatch(AppAction::DateFiltered(Some("2026-08-19".into())));
        let workspace = workspace(&model);
        assert_eq!(workspace.matching_events().len(), 2);
        assert_eq!(workspace.timeline_events().len(), 3);
        assert_eq!(workspace.timeline_date().as_deref(), Some("2026-08-19"));
    }

    #[test]
    fn actions_outside_the_workspace_are_ignored() {
        let mut model = AppModel::new();
        for action in [
            AppAction::CategoryFiltered(CategoryFilter::All),
            AppAction::DateFiltered(None),
            AppAction::DayChosen("2026-08-19".into()),
            AppAction::DayStepped(Step::Previous),
            AppAction::QueryFiltered("x".into()),
            AppAction::EventStepped(Step::Next),
            AppAction::LayoutChosen(StageLayout::Grid),
            AppAction::ScanProgressed {
                generation: 0,
                progress: ScanProgress::default(),
            },
            AppAction::ScanCancelled,
        ] {
            assert!(model.dispatch(action).is_empty());
            assert_eq!(model.state(), &AppState::Idle);
        }
    }
}
