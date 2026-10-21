//! Where the floating layers and the stage sit in the window (`DESIGN.md` §5 Desktop).
//!
//! The library floats 16 pt from the top, left and bottom (372 pt wide, 320 pt at 1280 pt and
//! below); the top row and the stage start right of it; the transport floats at the bottom of the
//! stage. Before a library is loaded (welcome, scanning, errors) the stage fills the floor.

use teslacam_core::theme::Theme;

use super::stage_layout::Rect;

/// Height left for the transport under the stage.
pub const TRANSPORT_ROOM: f32 = 124.0;
/// Space between the top row and the stage.
pub const TOP_ROW_GAP: f32 = 14.0;
/// Horizontal inset of the transport from the stage edges.
pub const TRANSPORT_INSET: f32 = 24.0;
/// Height of the transport (14 + 58 + 14).
pub const TRANSPORT_HEIGHT: f32 = 86.0;
/// Extra room the stage leaves while the transport shows a playback message.
pub const MESSAGE_ROOM: f32 = 40.0;
/// The message row: 10 pt (a 14 pt gap less 4) plus 18 pt per line (12 pt text, line height 1.5).
pub const MESSAGE_GAP: f32 = 10.0;
/// Height of one message line.
pub const MESSAGE_LINE: f32 = 18.0;

/// What the floor shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloorPhase {
    /// Welcome, scanning or an error: the stage is the car with six empty windows.
    Welcome,
    /// A library is loaded; `session` when an event is selected (top row and transport shown).
    Ready {
        /// Whether an event is selected.
        session: bool,
        /// Lines of the playback message under the transport (0 without one).
        message_lines: usize,
    },
}

/// The rectangles of the floor's layers, in window points.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloorLayout {
    /// The floating library.
    pub library: Rect,
    /// Event pill and actions pill row (once a library is loaded; without a selection it holds the
    /// placeholder pill).
    pub top_row: Option<Rect>,
    /// The stage.
    pub stage: Rect,
    /// The transport (only with a session).
    pub transport: Option<Rect>,
    /// Whether the window is at or below the compact-library width (narrower library, the event
    /// pill drops its detection).
    pub compact: bool,
}

/// The layout of a `width` × `height` window.
pub fn floor_layout(width: f32, height: f32, phase: FloorPhase, theme: &Theme) -> FloorLayout {
    let gutter = theme.spacing.gutter;
    let compact = width <= theme.breakpoints.compact_library_max;
    let library_width = if compact {
        theme.sizes.library_width_compact
    } else {
        theme.sizes.library_width
    };
    let library = Rect::new(
        gutter,
        gutter,
        library_width,
        (height - 2.0 * gutter).max(0.0),
    );
    let stage_left = library_width + 2.0 * gutter;
    let stage_width = (width - stage_left - gutter).max(0.0);
    let (session, message_lines) = match phase {
        FloorPhase::Ready {
            session,
            message_lines,
        } => (session, if session { message_lines } else { 0 }),
        FloorPhase::Welcome => (false, 0),
    };
    let top_row = matches!(phase, FloorPhase::Ready { .. })
        .then(|| Rect::new(stage_left, gutter, stage_width, theme.sizes.topbar_height));
    let stage = match phase {
        FloorPhase::Welcome => Rect::new(stage_left, gutter, stage_width, library.height),
        FloorPhase::Ready { .. } => {
            let top = gutter + theme.sizes.topbar_height + TOP_ROW_GAP;
            let bottom = match (session, message_lines) {
                (false, _) => gutter,
                (true, 0) => TRANSPORT_ROOM,
                (true, _) => TRANSPORT_ROOM + MESSAGE_ROOM,
            };
            Rect::new(
                stage_left,
                top,
                stage_width,
                (height - top - bottom).max(0.0),
            )
        }
    };
    let transport = session.then(|| {
        let available = (stage_width - 2.0 * TRANSPORT_INSET).max(0.0);
        let transport_width = available.min(theme.sizes.transport_max_width);
        let transport_height = if message_lines == 0 {
            TRANSPORT_HEIGHT
        } else {
            TRANSPORT_HEIGHT + MESSAGE_GAP + MESSAGE_LINE * message_lines as f32
        };
        Rect::new(
            stage_left + TRANSPORT_INSET + (available - transport_width) / 2.0,
            height - gutter - transport_height,
            transport_width,
            transport_height,
        )
    });
    FloorLayout {
        library,
        top_row,
        stage,
        transport,
        compact,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(width: f32, height: f32, phase: FloorPhase) -> FloorLayout {
        floor_layout(width, height, phase, Theme::embedded())
    }

    #[test]
    fn the_welcome_floor_gives_the_stage_everything_right_of_the_library() {
        let floor = layout(1440.0, 900.0, FloorPhase::Welcome);
        assert_eq!(floor.library, Rect::new(16.0, 16.0, 372.0, 868.0));
        assert_eq!(floor.stage, Rect::new(404.0, 16.0, 1020.0, 868.0));
        assert_eq!(floor.top_row, None);
        assert_eq!(floor.transport, None);
        assert!(!floor.compact);
    }

    #[test]
    fn a_session_adds_the_top_row_and_the_transport() {
        let floor = layout(
            1440.0,
            900.0,
            FloorPhase::Ready {
                session: true,
                message_lines: 0,
            },
        );
        assert_eq!(floor.top_row, Some(Rect::new(404.0, 16.0, 1020.0, 46.0)));
        assert_eq!(floor.stage, Rect::new(404.0, 76.0, 1020.0, 700.0));
        let transport = floor.transport.expect("transport");
        assert_eq!(transport.width, 920.0);
        assert_eq!(transport.center().0, floor.stage.center().0);
        assert_eq!(transport.bottom(), 884.0);
        let without = layout(
            1440.0,
            900.0,
            FloorPhase::Ready {
                session: false,
                message_lines: 0,
            },
        );
        assert_eq!(without.stage.bottom(), 884.0);
        assert_eq!(without.top_row, floor.top_row);
        assert_eq!(without.transport, None);
    }

    #[test]
    fn narrow_windows_use_the_compact_library() {
        let floor = layout(
            1100.0,
            700.0,
            FloorPhase::Ready {
                session: true,
                message_lines: 0,
            },
        );
        assert!(floor.compact);
        assert_eq!(floor.library.width, 320.0);
        assert_eq!(floor.stage.x, 352.0);
        assert_eq!(floor.stage.width, 732.0);
        assert_eq!(floor.transport.expect("transport").width, 684.0);
        assert!(layout(1280.0, 800.0, FloorPhase::Welcome).compact);
        assert!(!layout(1281.0, 800.0, FloorPhase::Welcome).compact);
    }

    #[test]
    fn a_playback_message_grows_the_transport_and_lifts_the_stage() {
        let floor = layout(
            1440.0,
            900.0,
            FloorPhase::Ready {
                session: true,
                message_lines: 2,
            },
        );
        assert_eq!(floor.stage.bottom(), 900.0 - 124.0 - 40.0);
        let transport = floor.transport.expect("transport");
        assert_eq!(transport.height, 86.0 + 10.0 + 2.0 * 18.0);
        assert_eq!(transport.bottom(), 884.0);
        let one_line = layout(
            1440.0,
            900.0,
            FloorPhase::Ready {
                session: true,
                message_lines: 1,
            },
        );
        assert_eq!(one_line.transport.expect("transport").height, 114.0);
    }
}
