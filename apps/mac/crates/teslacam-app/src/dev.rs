//! Development affordances read from the environment, for looking at the app without clicking
//! through it (screenshots, manual checks). None of them is needed in normal use.
//!
//! | Variable | Effect |
//! |---|---|
//! | `TESLACAM_OPEN=<folder>` | scans the folder at launch instead of waiting for the open panel |
//! | `TESLACAM_SELECT=<event id>` | selects that event once the library is loaded |
//! | `TESLACAM_VIEW=vehicle\|grid\|expanded:<camera>\|representative` | the stage view of the session |
//! | `TESLACAM_PLAY=1` | asks the session to play |
//! | `TESLACAM_SEEK=<seconds>` | seeks the session |
//! | `TESLACAM_UI=<item>,…` | opens UI: `details`, `technical`, `datepicker`, `search:<query>`, `cluster:<event id>`, `category:<recent\|saved\|sentry>`, `export` (the export dialog), `exportprogress` (the running export's popover), `rate` (the rate menu) |
//! | `TESLACAM_WINDOW=<w>x<h>` | window content size in points |
//! | `TESLACAM_CAPTURE=<png>` | captures the window with `screencapture` after a delay, then quits |
//! | `TESLACAM_CAPTURE_DELAY_MS=<ms>` | the delay before the capture (default 2500) |
//! | `TESLACAM_SCAN_DELAY_MS=<ms>` | slows the scan down by this much per progress report |
//! | `TESLACAM_EXPORT_TO=<mp4>` | starts the composite export of the selected event to this file (no save panel) |
//! | `TESLACAM_SYNC_PROBE=1` | logs how many frames apart the drawn cameras are (media fixtures only, see `sync_probe`) |

use std::path::PathBuf;
use std::time::Duration;

use teslacam_core::camera::{CameraId, ClipCategory};

/// The stage view to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DevView {
    /// `車両配置`
    Vehicle,
    /// `6分割`
    Grid,
    /// One camera expanded.
    Expanded(CameraId),
    /// `代表映像`
    Representative,
}

/// UI to open at launch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DevUi {
    /// The event details popover.
    Details,
    /// The details popover with `技術情報` open.
    Technical,
    /// The date picker.
    DatePicker,
    /// The search field with a query.
    Search(String),
    /// The rail cluster that contains this event.
    Cluster(String),
    /// A category filter.
    Category(ClipCategory),
    /// The export dialog.
    Export,
    /// The running export's popover.
    ExportProgress,
    /// The rate menu.
    Rate,
}

/// Development switches the views read (a GPUI global; absent in tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DevFlags {
    /// `TESLACAM_SYNC_PROBE`
    pub sync_probe: bool,
}

impl gpui_kit::Global for DevFlags {}

/// Everything read from the environment.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DevOptions {
    /// `TESLACAM_OPEN`
    pub open: Option<PathBuf>,
    /// `TESLACAM_SELECT`
    pub select: Option<String>,
    /// `TESLACAM_VIEW`
    pub view: Option<DevView>,
    /// `TESLACAM_PLAY`
    pub play: bool,
    /// `TESLACAM_SEEK`
    pub seek: Option<f64>,
    /// `TESLACAM_UI`
    pub ui: Vec<DevUi>,
    /// `TESLACAM_WINDOW`
    pub window_size: Option<(f32, f32)>,
    /// `TESLACAM_CAPTURE`
    pub capture: Option<PathBuf>,
    /// `TESLACAM_CAPTURE_DELAY_MS`
    pub capture_delay: Duration,
    /// `TESLACAM_SCAN_DELAY_MS`
    pub scan_delay: Duration,
    /// `TESLACAM_EXPORT_TO`
    pub export_to: Option<PathBuf>,
    /// `TESLACAM_SYNC_PROBE`
    pub sync_probe: bool,
}

fn parse_view(value: &str) -> Option<DevView> {
    match value {
        "vehicle" => Some(DevView::Vehicle),
        "grid" => Some(DevView::Grid),
        "representative" => Some(DevView::Representative),
        _ => CameraId::from_id(value.strip_prefix("expanded:")?).map(DevView::Expanded),
    }
}

fn parse_ui(item: &str) -> Option<DevUi> {
    match item {
        "details" => Some(DevUi::Details),
        "technical" => Some(DevUi::Technical),
        "datepicker" => Some(DevUi::DatePicker),
        "export" => Some(DevUi::Export),
        "exportprogress" => Some(DevUi::ExportProgress),
        "rate" => Some(DevUi::Rate),
        _ => {
            let (key, value) = item.split_once(':')?;
            match key {
                "search" => Some(DevUi::Search(value.to_owned())),
                "cluster" => Some(DevUi::Cluster(value.to_owned())),
                "category" => ClipCategory::from_id(value).map(DevUi::Category),
                _ => None,
            }
        }
    }
}

fn parse_size(value: &str) -> Option<(f32, f32)> {
    let (width, height) = value.split_once('x')?;
    Some((width.trim().parse().ok()?, height.trim().parse().ok()?))
}

fn millis(value: Option<String>, default: u64) -> Duration {
    Duration::from_millis(
        value
            .and_then(|value| value.trim().parse().ok())
            .unwrap_or(default),
    )
}

impl DevOptions {
    /// Reads the options through `get` (the environment in the app, a map in tests). Values that do
    /// not parse are ignored.
    pub fn from_env(get: impl Fn(&str) -> Option<String>) -> DevOptions {
        let non_empty = |key: &str| get(key).filter(|value| !value.trim().is_empty());
        DevOptions {
            open: non_empty("TESLACAM_OPEN").map(PathBuf::from),
            select: non_empty("TESLACAM_SELECT"),
            view: non_empty("TESLACAM_VIEW").and_then(|value| parse_view(value.trim())),
            play: non_empty("TESLACAM_PLAY").is_some_and(|value| value.trim() == "1"),
            seek: non_empty("TESLACAM_SEEK").and_then(|value| value.trim().parse().ok()),
            ui: non_empty("TESLACAM_UI")
                .map(|value| {
                    value
                        .split(',')
                        .filter_map(|item| parse_ui(item.trim()))
                        .collect()
                })
                .unwrap_or_default(),
            window_size: non_empty("TESLACAM_WINDOW").and_then(|value| parse_size(&value)),
            capture: non_empty("TESLACAM_CAPTURE").map(PathBuf::from),
            capture_delay: millis(non_empty("TESLACAM_CAPTURE_DELAY_MS"), 2_500),
            scan_delay: millis(non_empty("TESLACAM_SCAN_DELAY_MS"), 0),
            export_to: non_empty("TESLACAM_EXPORT_TO").map(PathBuf::from),
            sync_probe: non_empty("TESLACAM_SYNC_PROBE").is_some_and(|value| value.trim() == "1"),
        }
    }

    /// The options of this process.
    pub fn from_process() -> DevOptions {
        DevOptions::from_env(|key| std::env::var(key).ok())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn options(pairs: &[(&str, &str)]) -> DevOptions {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect();
        DevOptions::from_env(|key| map.get(key).cloned())
    }

    #[test]
    fn nothing_set_means_normal_use() {
        let options = options(&[]);
        assert_eq!(options.open, None);
        assert!(options.ui.is_empty());
        assert_eq!(options.capture_delay, Duration::from_millis(2_500));
        assert_eq!(options.scan_delay, Duration::ZERO);
    }

    #[test]
    fn every_variable_parses() {
        let options = options(&[
            ("TESLACAM_OPEN", "/Volumes/TESLA/TeslaCam"),
            ("TESLACAM_SELECT", "sentry-2026-08-20_20-35-00-1"),
            ("TESLACAM_VIEW", "expanded:left_pillar"),
            ("TESLACAM_PLAY", "1"),
            ("TESLACAM_SEEK", "42.5"),
            (
                "TESLACAM_UI",
                "details, search:20:35,cluster:saved-1,category:sentry,bogus,export,rate",
            ),
            ("TESLACAM_EXPORT_TO", "/tmp/out.mp4"),
            ("TESLACAM_SYNC_PROBE", "1"),
            ("TESLACAM_WINDOW", "1100x700"),
            ("TESLACAM_CAPTURE", "/tmp/shot.png"),
            ("TESLACAM_CAPTURE_DELAY_MS", "800"),
            ("TESLACAM_SCAN_DELAY_MS", "30"),
        ]);
        assert_eq!(
            options.open.as_deref(),
            Some(std::path::Path::new("/Volumes/TESLA/TeslaCam"))
        );
        assert_eq!(options.view, Some(DevView::Expanded(CameraId::LeftPillar)));
        assert!(options.play);
        assert_eq!(options.seek, Some(42.5));
        assert_eq!(
            options.ui,
            [
                DevUi::Details,
                DevUi::Search("20:35".into()),
                DevUi::Cluster("saved-1".into()),
                DevUi::Category(ClipCategory::Sentry),
                DevUi::Export,
                DevUi::Rate
            ]
        );
        assert_eq!(
            options.export_to.as_deref(),
            Some(std::path::Path::new("/tmp/out.mp4"))
        );
        assert!(options.sync_probe);
        assert_eq!(options.window_size, Some((1100.0, 700.0)));
        assert_eq!(options.capture_delay, Duration::from_millis(800));
        assert_eq!(options.scan_delay, Duration::from_millis(30));
    }

    #[test]
    fn views_parse() {
        assert_eq!(parse_view("grid"), Some(DevView::Grid));
        assert_eq!(parse_view("representative"), Some(DevView::Representative));
        assert_eq!(parse_view("expanded:cabin"), None);
        assert_eq!(parse_view("Grid"), None);
    }
}
