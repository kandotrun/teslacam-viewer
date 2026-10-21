//! The GPUI views. They read the [`AppStore`](crate::store::AppStore) and send it actions; layout,
//! copy and state rules come from `teslacam-core` and [`crate::presentation`].

pub mod components;
pub mod date_picker;
pub mod library;
pub mod rail;
pub mod root;
pub mod stage;
pub mod style;
#[cfg(test)]
mod tests;
pub mod thumbnails;
pub mod top_row;

use teslacam_core::app_state::AppState;

use crate::presentation::floor_layout::FloorPhase;
use crate::store::AppStore;

/// The floor phase of the store's state.
pub fn floor_phase(store: &AppStore) -> FloorPhase {
    match store.state() {
        AppState::Ready(_) => FloorPhase::Ready {
            session: store.session().is_some(),
        },
        _ => FloorPhase::Welcome,
    }
}
