//! Theater keyboard shortcuts (`DESIGN.md` §9).
//!
//! | Key | Action |
//! |---|---|
//! | `Space` | play / pause |
//! | `←` / `→` | −10 s / +10 s |
//! | `1`–`6` | expand that camera (front, back, left pillar, left repeater, right pillar, right repeater); the same key again collapses |
//! | `Esc` | collapse the expanded camera (also while a control has focus) |
//! | `[` / `]` | previous (older) / next (newer) event |
//!
//! Keys are ignored while a text field or a button has keyboard focus (except `Esc`), while the
//! export dialog is open, and whenever ⌘, ⌃ or ⌥ is held (those belong to menus).

use crate::camera::CameraId;

/// Seconds skipped by `←` / `→`.
pub const SKIP_SECONDS: f64 = 10.0;

/// Modifier keys held with a key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    /// ⌘
    pub command: bool,
    /// ⌃
    pub control: bool,
    /// ⌥
    pub alt: bool,
    /// ⇧ (does not block shortcuts; the key name already reflects it)
    pub shift: bool,
}

/// What holds keyboard focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusTarget {
    /// Nothing that consumes keys.
    #[default]
    None,
    /// A button (camera windows, transport buttons, rail markers included).
    Button,
    /// A text field, date picker, menu or other editable control.
    TextField,
}

/// The state the shortcuts depend on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShortcutContext {
    /// What holds keyboard focus.
    pub focus: FocusTarget,
    /// The expanded camera, if any.
    pub expanded_camera: Option<CameraId>,
    /// Whether the export dialog is open.
    pub dialog_open: bool,
}

/// What a shortcut asks for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShortcutAction {
    /// Play when paused, pause when playing.
    TogglePlay,
    /// Seek by this many seconds.
    Skip(f64),
    /// Make the camera active and toggle its expansion.
    ToggleCamera(CameraId),
    /// Collapse the expanded camera.
    Collapse,
    /// Select the next older event.
    PreviousEvent,
    /// Select the next newer event.
    NextEvent,
}

/// Maps a key press to a theater action. `key` uses GPUI key names (`space`, `left`, `right`,
/// `escape`, `1`…`6`, `[`, `]`). `Some` means the key is consumed.
pub fn theater_shortcut(
    key: &str,
    modifiers: Modifiers,
    context: ShortcutContext,
) -> Option<ShortcutAction> {
    if modifiers.command || modifiers.control || modifiers.alt || context.dialog_open {
        return None;
    }
    if key == "escape" {
        return context.expanded_camera.map(|_| ShortcutAction::Collapse);
    }
    if context.focus != FocusTarget::None {
        return None;
    }
    match key {
        "space" => Some(ShortcutAction::TogglePlay),
        "left" => Some(ShortcutAction::Skip(-SKIP_SECONDS)),
        "right" => Some(ShortcutAction::Skip(SKIP_SECONDS)),
        "[" => Some(ShortcutAction::PreviousEvent),
        "]" => Some(ShortcutAction::NextEvent),
        _ => {
            let digit = key.parse::<u8>().ok().filter(|_| key.len() == 1)?;
            CameraId::from_digit(digit).map(ShortcutAction::ToggleCamera)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(key: &str) -> Option<ShortcutAction> {
        theater_shortcut(key, Modifiers::default(), ShortcutContext::default())
    }

    #[test]
    fn transport_keys() {
        assert_eq!(press("space"), Some(ShortcutAction::TogglePlay));
        assert_eq!(press("left"), Some(ShortcutAction::Skip(-10.0)));
        assert_eq!(press("right"), Some(ShortcutAction::Skip(10.0)));
        assert_eq!(press("["), Some(ShortcutAction::PreviousEvent));
        assert_eq!(press("]"), Some(ShortcutAction::NextEvent));
    }

    #[test]
    fn digits_toggle_cameras_in_canonical_order() {
        let cameras = ["1", "2", "3", "4", "5", "6"].map(press);
        assert_eq!(
            cameras,
            CameraId::ALL.map(|camera| Some(ShortcutAction::ToggleCamera(camera)))
        );
        assert_eq!(press("0"), None);
        assert_eq!(press("7"), None);
    }

    #[test]
    fn escape_collapses_only_an_expanded_camera() {
        assert_eq!(press("escape"), None);
        let expanded = ShortcutContext {
            expanded_camera: Some(CameraId::RightRepeater),
            ..ShortcutContext::default()
        };
        assert_eq!(
            theater_shortcut("escape", Modifiers::default(), expanded),
            Some(ShortcutAction::Collapse)
        );
        let focused = ShortcutContext {
            focus: FocusTarget::Button,
            ..expanded
        };
        assert_eq!(
            theater_shortcut("escape", Modifiers::default(), focused),
            Some(ShortcutAction::Collapse)
        );
    }

    #[test]
    fn focused_controls_keep_their_keys() {
        for focus in [FocusTarget::Button, FocusTarget::TextField] {
            let context = ShortcutContext {
                focus,
                ..ShortcutContext::default()
            };
            for key in ["space", "left", "right", "6", "[", "]"] {
                assert_eq!(
                    theater_shortcut(key, Modifiers::default(), context),
                    None,
                    "{key} with {focus:?}"
                );
            }
        }
    }

    #[test]
    fn menu_modifiers_are_ignored() {
        for modifiers in [
            Modifiers {
                command: true,
                ..Modifiers::default()
            },
            Modifiers {
                control: true,
                ..Modifiers::default()
            },
            Modifiers {
                alt: true,
                ..Modifiers::default()
            },
        ] {
            for key in ["1", "6", "left", "right", "space", "[", "]"] {
                assert_eq!(
                    theater_shortcut(key, modifiers, ShortcutContext::default()),
                    None,
                    "{key} with {modifiers:?}"
                );
            }
        }
        let shift = Modifiers {
            shift: true,
            ..Modifiers::default()
        };
        assert_eq!(
            theater_shortcut("space", shift, ShortcutContext::default()),
            Some(ShortcutAction::TogglePlay)
        );
    }

    #[test]
    fn the_export_dialog_suspends_shortcuts() {
        let context = ShortcutContext {
            dialog_open: true,
            expanded_camera: Some(CameraId::Front),
            ..ShortcutContext::default()
        };
        for key in ["space", "1", "escape", "right"] {
            assert_eq!(
                theater_shortcut(key, Modifiers::default(), context),
                None,
                "{key}"
            );
        }
    }

    #[test]
    fn unknown_keys_are_not_consumed() {
        assert_eq!(press("a"), None);
        assert_eq!(press("enter"), None);
        assert_eq!(press(""), None);
    }
}
