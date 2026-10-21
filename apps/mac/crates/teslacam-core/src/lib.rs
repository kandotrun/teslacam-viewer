//! TeslaCam domain rules shared with the web app through `packages/contract`.
//!
//! Everything here is pure Rust without platform dependencies: the GPUI app (`teslacam-app`) and the
//! AVFoundation layer (`teslacam-media`) build on these types and state machines. Behaviour that the
//! web app also has is pinned by the golden fixtures in `packages/contract/fixtures`; Mac-only rules
//! are documented on the items and in `packages/contract/README.md`.
//!
//! | Area | Modules |
//! |---|---|
//! | Cameras, timestamps, file names | [`camera`], [`timestamp`], [`filename`] |
//! | Folder scan and library | [`limits`], [`metadata`], [`library`], [`scan`] |
//! | Event presentation | [`filters`], [`summary`], [`detection`], [`copy`] |
//! | Time axes | [`timeline`] (the web's nominal 60 s axis), [`event_timeline`] (the Mac's wall-clock axis), [`day_timeline`] |
//! | State machines | [`app_state`], [`playback`], [`export_state`], [`shortcuts`] |
//! | Export and design tokens | [`export_plan`], [`theme`] |

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod app_state;
pub mod camera;
pub mod copy;
pub mod day_timeline;
pub mod detection;
pub mod event_timeline;
pub mod export_plan;
pub mod export_state;
pub mod filename;
pub mod filters;
pub mod library;
pub mod limits;
pub mod metadata;
pub mod playback;
pub mod scan;
pub mod shortcuts;
pub mod summary;
pub mod theme;
pub mod timeline;
pub mod timestamp;

#[cfg(test)]
mod contract;
