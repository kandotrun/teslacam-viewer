//! The composite export control of one theater session (spec §2.11, `DESIGN.md` actions pill), in
//! its Mac form: the destination is chosen with the save panel before the export starts, and the
//! finished file is shown in Finder instead of being saved afterwards.
//!
//! ```text
//! Idle ──Open──▶ Confirm ──Start──▶ ChoosingDestination ──DestinationChosen──▶ Running{progress}
//!   ▲              │  ▲                    │                                    │
//!   └──Dismiss─────┘  └─DestinationCancelled┘   Done{output} ◀──Completed────────┤
//!                                               │  ▲ Reveal                       ├──Failed──▶ Failed
//!   Idle ◀──Close───────────────────────────────┘  └─┘                            └──Abort───▶ Cancelled
//! Failed / Cancelled ──Open──▶ Confirm
//! ```
//!
//! The machine owns no media: it emits [`ExportEffect`]s for the app. Ending the session (another
//! event selected, a rescan) aborts a running export silently; a finished file is the user's and is
//! never deleted. A file that completes after an abort is discarded (an aborted export never leaves
//! a file behind).

use std::path::PathBuf;

use crate::copy;
use crate::library::Event;
use crate::summary::camera_availability;

/// Whether `event` can be exported: some camera has footage in every segment (spec A-7).
pub fn export_available(event: &Event) -> bool {
    camera_availability(event).available > 0
}

/// Where the export control is.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportState {
    /// The `書き出す` action is shown.
    Idle,
    /// The dialog is open.
    Confirm,
    /// `書き出しを開始` was pressed; the save panel asks where the file goes (the dialog stays).
    ChoosingDestination,
    /// Exporting; progress 0–1, never decreasing.
    Running {
        /// Fraction done.
        progress: f64,
    },
    /// The file is complete at the chosen destination (`書き出し完了`, `Finderで表示`).
    Done {
        /// The finished file.
        output: PathBuf,
    },
    /// The export failed with a displayable message.
    Failed {
        /// The message.
        message: String,
    },
    /// The user aborted; `書き出しを中止しました。` is shown.
    Cancelled,
}

/// Inputs from the UI and the media layer.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportAction {
    /// `書き出す`, or `もう一度` after a failure: opens the dialog.
    Open,
    /// `キャンセル`, the close button or `Esc` in the dialog.
    Dismiss,
    /// `書き出しを開始`.
    Start,
    /// The save panel returned a destination.
    DestinationChosen(PathBuf),
    /// The save panel was cancelled.
    DestinationCancelled,
    /// Progress from the media layer.
    Progressed(f64),
    /// `中止`.
    Abort,
    /// The media layer finished the file.
    Completed(PathBuf),
    /// The media layer failed; `None` shows the generic message.
    Failed(Option<String>),
    /// `Finderで表示`.
    Reveal,
    /// `閉じる` after completion.
    Close,
    /// The theater session ended.
    SessionEnded,
}

/// Work for the app and the media layer.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportEffect {
    /// Show the save panel named [`copy::export_file_name`].
    ChooseDestination,
    /// Start exporting the session's event to this file.
    StartExport(PathBuf),
    /// Stop the running export and delete partial output.
    CancelExport,
    /// Show the finished file in Finder.
    RevealOutput(PathBuf),
    /// Delete a file that completed after the export was aborted.
    DiscardOutput(PathBuf),
}

/// The export control.
#[derive(Debug, Clone, PartialEq)]
pub struct ExportMachine {
    state: ExportState,
    available: bool,
}

impl ExportMachine {
    /// A new control; `available` is whether some camera has footage in every segment.
    pub fn new(available: bool) -> ExportMachine {
        ExportMachine {
            state: ExportState::Idle,
            available,
        }
    }

    /// The current state.
    pub fn state(&self) -> &ExportState {
        &self.state
    }

    /// Whether the dialog is shown (theater shortcuts are suspended then), including while the save
    /// panel is up.
    pub fn dialog_open(&self) -> bool {
        matches!(
            self.state,
            ExportState::Confirm | ExportState::ChoosingDestination
        )
    }

    /// The running export's progress.
    pub fn progress(&self) -> Option<f64> {
        match self.state {
            ExportState::Running { progress } => Some(progress),
            _ => None,
        }
    }

    /// Why the export cannot start, shown next to the disabled action.
    pub fn unavailable_reason(&self) -> Option<&'static str> {
        (!self.available).then_some(copy::EXPORT_UNAVAILABLE)
    }

    /// The action's text: `書き出す`, `書き出し中 42%` or `書き出し完了`.
    pub fn action_label(&self) -> String {
        match &self.state {
            ExportState::Running { progress } => copy::export_running_text(*progress),
            ExportState::Done { .. } => copy::EXPORT_DONE.to_owned(),
            _ => copy::EXPORT.to_owned(),
        }
    }

    /// The status text shown under the action (abort notice or error message).
    pub fn notice(&self) -> Option<&str> {
        match &self.state {
            ExportState::Cancelled => Some(copy::EXPORT_ABORTED),
            ExportState::Failed { message } => Some(message),
            _ => None,
        }
    }

    /// Applies `action` and returns the effects.
    pub fn dispatch(&mut self, action: ExportAction) -> Vec<ExportEffect> {
        use ExportAction as Action;
        use ExportState as State;
        let (next, effects) = match (&self.state, action) {
            (State::Idle | State::Cancelled | State::Failed { .. }, Action::Open)
                if self.available =>
            {
                (State::Confirm, vec![])
            }
            (State::Confirm, Action::Dismiss) => (State::Idle, vec![]),
            (State::Confirm, Action::Start) => (
                State::ChoosingDestination,
                vec![ExportEffect::ChooseDestination],
            ),
            (State::ChoosingDestination, Action::DestinationCancelled) => (State::Confirm, vec![]),
            (State::ChoosingDestination, Action::DestinationChosen(output)) => (
                State::Running { progress: 0.0 },
                vec![ExportEffect::StartExport(output)],
            ),
            (State::Running { progress }, Action::Progressed(update)) => {
                let progress = if update.is_nan() {
                    *progress
                } else {
                    progress.max(update.clamp(0.0, 1.0))
                };
                (State::Running { progress }, vec![])
            }
            (State::Running { .. }, Action::Abort) => {
                (State::Cancelled, vec![ExportEffect::CancelExport])
            }
            (State::Running { .. }, Action::Completed(output)) => (State::Done { output }, vec![]),
            (State::Running { .. }, Action::Failed(message)) => (
                State::Failed {
                    message: message.unwrap_or_else(|| copy::EXPORT_ERROR_GENERIC.to_owned()),
                },
                vec![],
            ),
            (State::Done { output }, Action::Reveal) => (
                self.state.clone(),
                vec![ExportEffect::RevealOutput(output.clone())],
            ),
            (State::Done { .. }, Action::Close) => (State::Idle, vec![]),
            // A repeated completion must never discard the user's finished file.
            (State::Done { .. }, Action::Completed(_)) => (self.state.clone(), vec![]),
            (State::Running { .. }, Action::SessionEnded) => {
                (State::Idle, vec![ExportEffect::CancelExport])
            }
            (_, Action::SessionEnded) => (State::Idle, vec![]),
            (_, Action::Completed(output)) => (
                self.state.clone(),
                vec![ExportEffect::DiscardOutput(output)],
            ),
            _ => (self.state.clone(), vec![]),
        };
        self.state = next;
        effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn destination() -> PathBuf {
        PathBuf::from("/Users/me/Movies/TeslaCam-2026-08-20_20-45-00-vehicle.mp4")
    }

    fn running(machine: &mut ExportMachine) {
        machine.dispatch(ExportAction::Open);
        assert_eq!(
            machine.dispatch(ExportAction::Start),
            vec![ExportEffect::ChooseDestination]
        );
        assert_eq!(
            machine.dispatch(ExportAction::DestinationChosen(destination())),
            vec![ExportEffect::StartExport(destination())]
        );
    }

    #[test]
    fn availability_needs_a_camera_in_every_segment() {
        use crate::contract::event_input;
        let event = |segments: serde_json::Value| {
            event_input(
                &serde_json::json!({"id": "e", "category": "saved", "timestamp": "2026-08-20_20-10-00", "segments": segments}),
            )
        };
        assert!(export_available(&event(serde_json::json!([
            {"timestamp": "2026-08-20_20-10-00", "cameras": ["front", "back"]},
            {"timestamp": "2026-08-20_20-11-00", "cameras": ["front"]},
        ]))));
        assert!(!export_available(&event(serde_json::json!([
            {"timestamp": "2026-08-20_20-10-00", "cameras": ["front"]},
            {"timestamp": "2026-08-20_20-11-00", "cameras": ["back"]},
        ]))));
        assert!(!export_available(&event(serde_json::json!([]))));
    }

    #[test]
    fn the_dialog_opens_and_closes() {
        let mut machine = ExportMachine::new(true);
        assert_eq!(machine.action_label(), "書き出す");
        assert!(machine.dispatch(ExportAction::Open).is_empty());
        assert_eq!(machine.state(), &ExportState::Confirm);
        assert!(machine.dialog_open());
        assert!(machine.dispatch(ExportAction::Dismiss).is_empty());
        assert_eq!(machine.state(), &ExportState::Idle);
        assert!(!machine.dialog_open());
    }

    #[test]
    fn starting_asks_for_the_destination_first() {
        let mut machine = ExportMachine::new(true);
        machine.dispatch(ExportAction::Open);
        assert_eq!(
            machine.dispatch(ExportAction::Start),
            vec![ExportEffect::ChooseDestination]
        );
        assert_eq!(machine.state(), &ExportState::ChoosingDestination);
        // The dialog stays up (and the shortcuts suspended) while the save panel is shown.
        assert!(machine.dialog_open());
        assert!(machine.dispatch(ExportAction::Start).is_empty());
        assert!(machine.dispatch(ExportAction::Dismiss).is_empty());
        // Cancelling the save panel returns to the dialog.
        assert!(
            machine
                .dispatch(ExportAction::DestinationCancelled)
                .is_empty()
        );
        assert_eq!(machine.state(), &ExportState::Confirm);
        machine.dispatch(ExportAction::Start);
        assert_eq!(
            machine.dispatch(ExportAction::DestinationChosen(destination())),
            vec![ExportEffect::StartExport(destination())]
        );
        assert_eq!(machine.state(), &ExportState::Running { progress: 0.0 });
        assert!(!machine.dialog_open());
    }

    #[test]
    fn an_unavailable_export_never_opens() {
        let mut machine = ExportMachine::new(false);
        machine.dispatch(ExportAction::Open);
        assert_eq!(machine.state(), &ExportState::Idle);
        assert_eq!(
            machine.unavailable_reason(),
            Some("6カメラ映像がないため書き出せません")
        );
        assert_eq!(ExportMachine::new(true).unavailable_reason(), None);
    }

    #[test]
    fn progress_is_clamped_and_never_decreases() {
        let mut machine = ExportMachine::new(true);
        running(&mut machine);
        machine.dispatch(ExportAction::Progressed(0.42));
        assert_eq!(machine.action_label(), "書き出し中 42%");
        assert_eq!(machine.progress(), Some(0.42));
        machine.dispatch(ExportAction::Progressed(0.3));
        machine.dispatch(ExportAction::Progressed(f64::NAN));
        assert_eq!(machine.state(), &ExportState::Running { progress: 0.42 });
        machine.dispatch(ExportAction::Progressed(7.0));
        assert_eq!(machine.state(), &ExportState::Running { progress: 1.0 });
        assert_eq!(ExportMachine::new(true).progress(), None);
    }

    #[test]
    fn completion_offers_the_file_in_finder_until_closed() {
        let mut machine = ExportMachine::new(true);
        running(&mut machine);
        machine.dispatch(ExportAction::Completed(destination()));
        assert_eq!(machine.action_label(), "書き出し完了");
        assert_eq!(machine.notice(), None);
        assert!(
            machine
                .dispatch(ExportAction::Completed(destination()))
                .is_empty()
        );
        assert_eq!(
            machine.dispatch(ExportAction::Reveal),
            vec![ExportEffect::RevealOutput(destination())]
        );
        assert_eq!(
            machine.dispatch(ExportAction::Reveal),
            vec![ExportEffect::RevealOutput(destination())]
        );
        // The file is the user's: closing never deletes it.
        assert!(machine.dispatch(ExportAction::Close).is_empty());
        assert_eq!(machine.state(), &ExportState::Idle);
        assert!(machine.dispatch(ExportAction::Reveal).is_empty());
    }

    #[test]
    fn aborting_shows_a_notice_and_never_keeps_a_file() {
        let mut machine = ExportMachine::new(true);
        running(&mut machine);
        assert_eq!(
            machine.dispatch(ExportAction::Abort),
            vec![ExportEffect::CancelExport]
        );
        assert_eq!(machine.state(), &ExportState::Cancelled);
        assert_eq!(machine.notice(), Some("書き出しを中止しました。"));
        let late = PathBuf::from("/tmp/late.mp4");
        assert_eq!(
            machine.dispatch(ExportAction::Completed(late.clone())),
            vec![ExportEffect::DiscardOutput(late)]
        );
        assert_eq!(machine.state(), &ExportState::Cancelled);
        assert!(machine.dispatch(ExportAction::Failed(None)).is_empty());
        machine.dispatch(ExportAction::Open);
        assert_eq!(machine.state(), &ExportState::Confirm);
    }

    #[test]
    fn failures_show_the_message_and_retry_opens_the_dialog() {
        let mut machine = ExportMachine::new(true);
        running(&mut machine);
        machine.dispatch(ExportAction::Failed(Some(
            copy::EXPORT_ERROR_NO_MEDIA.to_owned(),
        )));
        assert_eq!(machine.notice(), Some("再生可能な動画がありません。"));
        assert_eq!(machine.action_label(), "書き出す");
        machine.dispatch(ExportAction::Open);
        assert_eq!(machine.state(), &ExportState::Confirm);
        machine.dispatch(ExportAction::Start);
        machine.dispatch(ExportAction::DestinationChosen(destination()));
        machine.dispatch(ExportAction::Failed(None));
        assert_eq!(
            machine.notice(),
            Some("合成動画を書き出せませんでした。もう一度お試しください。")
        );
    }

    #[test]
    fn ending_the_session_aborts_silently_and_keeps_finished_files() {
        let mut machine = ExportMachine::new(true);
        running(&mut machine);
        assert_eq!(
            machine.dispatch(ExportAction::SessionEnded),
            vec![ExportEffect::CancelExport]
        );
        assert_eq!(machine.state(), &ExportState::Idle);
        assert_eq!(machine.notice(), None);

        let mut finished = ExportMachine::new(true);
        running(&mut finished);
        finished.dispatch(ExportAction::Completed(destination()));
        assert!(finished.dispatch(ExportAction::SessionEnded).is_empty());
        assert_eq!(finished.state(), &ExportState::Idle);

        let mut choosing = ExportMachine::new(true);
        choosing.dispatch(ExportAction::Open);
        choosing.dispatch(ExportAction::Start);
        assert!(choosing.dispatch(ExportAction::SessionEnded).is_empty());
        assert_eq!(choosing.state(), &ExportState::Idle);
        // A destination that arrives after the session ended starts nothing.
        assert!(
            choosing
                .dispatch(ExportAction::DestinationChosen(destination()))
                .is_empty()
        );
        assert!(
            ExportMachine::new(true)
                .dispatch(ExportAction::SessionEnded)
                .is_empty()
        );
    }

    #[test]
    fn inapplicable_actions_are_ignored() {
        let mut machine = ExportMachine::new(true);
        for action in [
            ExportAction::Start,
            ExportAction::Abort,
            ExportAction::Reveal,
            ExportAction::Close,
            ExportAction::Progressed(0.5),
            ExportAction::DestinationCancelled,
            ExportAction::DestinationChosen(destination()),
        ] {
            assert!(machine.dispatch(action).is_empty());
            assert_eq!(machine.state(), &ExportState::Idle);
        }
        running(&mut machine);
        assert!(machine.dispatch(ExportAction::Open).is_empty());
        assert!(machine.dispatch(ExportAction::Dismiss).is_empty());
        assert_eq!(machine.state(), &ExportState::Running { progress: 0.0 });
    }
}
