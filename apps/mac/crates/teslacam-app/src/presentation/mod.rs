//! Pure presentation logic of the app: geometry, motion, text runs and cache bookkeeping. Nothing
//! here depends on GPUI, so it is unit-tested directly.

pub mod bitmap;
pub mod floor_art;
pub mod floor_layout;
pub mod lru;
pub mod motion;
pub mod numerals;
pub mod stage_layout;
pub mod transport;
