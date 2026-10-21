//! The composite export control of one theater session (spec §2.11, `DESIGN.md` actions pill).
//!
//! ```text
//! Idle ──Open──▶ Confirm ──Start──▶ Running{progress} ──Completed──▶ Done{output} ──Close──▶ Idle
//!   ▲              │                   │  └──Failed──▶ Failed{message} ──Open──▶ Confirm
//!   └──Dismiss─────┘                   └──Abort──▶ Cancelled ──Open──▶ Confirm
//! ```
//!
//! The machine owns no media: it emits [`ExportEffect`]s for the media layer. Ending the session
//! (another event selected, a rescan) aborts a running export silently and discards a finished one.

use std::path::PathBuf;

use crate::copy;

/// Where the export control is.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportState {
    /// The `書き出す` action is shown.
    Idle,
    /// The dialog is open.
    Confirm,
    /// Exporting; progress 0–1, never decreasing.
    Running {
        /// Fraction done.
        progress: f64,
    },
    /// A finished file waits to be saved (repeatedly) until closed.
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
    /// Progress from the media layer.
    Progressed(f64),
    /// `中止`.
    Abort,
    /// The media layer finished the file.
    Completed(PathBuf),
    /// The media layer failed; `None` shows the generic message.
    Failed(Option<String>),
    /// `MP4を保存`.
    Save,
    /// `閉じる` after completion.
    Close,
    /// The theater session ended.
    SessionEnded,
}

/// Work for the media layer.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportEffect {
    /// Start exporting the session's event.
    StartExport,
    /// Stop the running export and delete partial output.
    CancelExport,
    /// Offer `output` in a save panel named [`copy::export_file_name`].
    SaveOutput(PathBuf),
    /// Delete a finished file that is no longer offered.
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

    /// Whether the dialog is shown (theater shortcuts are suspended then).
    pub fn dialog_open(&self) -> bool {
        self.state == ExportState::Confirm
    }

    /// Why the export cannot start, shown next to the disabled action.
    pub fn unavailable_reason(&self) -> Option<&'static str> {
        (!self.available).then_some(copy::EXPORT_UNAVAILABLE)
    }

    /// The action's text: `書き出す`, `書き出し中 42%` or `MP4を保存`.
    pub fn action_label(&self) -> String {
        match &self.state {
            ExportState::Running { progress } => copy::export_running_text(*progress),
            ExportState::Done { .. } => copy::export_save_label("mp4"),
            _ => copy::EXPORT.to_owned(),
        }
    }

    /// The status text next to the action.
    pub fn notice(&self) -> Option<&str> {
        match &self.state {
            ExportState::Cancelled => Some(copy::EXPORT_ABORTED),
            ExportState::Failed { message } => Some(message),
            ExportState::Done { .. } => Some(copy::EXPORT_DONE),
            _ => None,
        }
    }

    /// Applies `action` and returns the effects for the media layer.
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
                State::Running { progress: 0.0 },
                vec![ExportEffect::StartExport],
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
            (State::Done { output }, Action::Save) => (
                self.state.clone(),
                vec![ExportEffect::SaveOutput(output.clone())],
            ),
            (State::Done { output }, Action::Close) => (
                State::Idle,
                vec![ExportEffect::DiscardOutput(output.clone())],
            ),
            (State::Running { .. }, Action::SessionEnded) => {
                (State::Idle, vec![ExportEffect::CancelExport])
            }
            (State::Done { output }, Action::SessionEnded) => (
                State::Idle,
                vec![ExportEffect::DiscardOutput(output.clone())],
            ),
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

    fn running(machine: &mut ExportMachine) {
        machine.dispatch(ExportAction::Open);
        assert_eq!(
            machine.dispatch(ExportAction::Start),
            vec![ExportEffect::StartExport]
        );
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
        assert_eq!(machine.state(), &ExportState::Running { progress: 0.0 });
        machine.dispatch(ExportAction::Progressed(0.42));
        assert_eq!(machine.action_label(), "書き出し中 42%");
        machine.dispatch(ExportAction::Progressed(0.3));
        machine.dispatch(ExportAction::Progressed(f64::NAN));
        assert_eq!(machine.state(), &ExportState::Running { progress: 0.42 });
        machine.dispatch(ExportAction::Progressed(7.0));
        assert_eq!(machine.state(), &ExportState::Running { progress: 1.0 });
    }

    #[test]
    fn completion_offers_the_file_until_closed() {
        let mut machine = ExportMachine::new(true);
        running(&mut machine);
        let output = PathBuf::from("/tmp/export.mp4");
        machine.dispatch(ExportAction::Completed(output.clone()));
        assert_eq!(machine.action_label(), "MP4を保存");
        assert_eq!(machine.notice(), Some("書き出し完了"));
        assert_eq!(
            machine.dispatch(ExportAction::Save),
            vec![ExportEffect::SaveOutput(output.clone())]
        );
        assert_eq!(
            machine.dispatch(ExportAction::Save),
            vec![ExportEffect::SaveOutput(output.clone())]
        );
        assert_eq!(
            machine.dispatch(ExportAction::Close),
            vec![ExportEffect::DiscardOutput(output)]
        );
        assert_eq!(machine.state(), &ExportState::Idle);
    }

    #[test]
    fn aborting_shows_a_notice_and_never_offers_a_file() {
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
        machine.dispatch(ExportAction::Open);
        assert_eq!(machine.state(), &ExportState::Confirm);
        machine.dispatch(ExportAction::Start);
        machine.dispatch(ExportAction::Failed(None));
        assert_eq!(
            machine.notice(),
            Some("合成動画を書き出せませんでした。もう一度お試しください。")
        );
    }

    #[test]
    fn ending_the_session_aborts_silently() {
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
        let output = PathBuf::from("/tmp/done.mp4");
        finished.dispatch(ExportAction::Completed(output.clone()));
        assert_eq!(
            finished.dispatch(ExportAction::SessionEnded),
            vec![ExportEffect::DiscardOutput(output)]
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
            ExportAction::Save,
            ExportAction::Close,
            ExportAction::Progressed(0.5),
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
