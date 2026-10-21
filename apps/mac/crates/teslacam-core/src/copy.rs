//! Every Japanese string the Mac UI renders, as constants and formatters.
//!
//! Sources, in order of precedence: `DESIGN.md` (garage floor), the approved prototype
//! `docs/design/2026-10-20-garage-floor.html` where `DESIGN.md` names an element without its text,
//! and `docs/specs/2026-10-20-mac-native-parity.md` §5 for everything the redesign kept. Browser-only
//! lines (browser guidance, the browser half of the rejected-playback message, the "keep the tab open"
//! export notice) are not rendered on the Mac.
//!
//! The formatters shared with the web are pinned by `packages/contract/fixtures/copy-format.json`.
//! Punctuation is part of the contract: `、` U+3001, `・` U+30FB, `–` U+2013, `×` U+00D7, full-width
//! `（）` and half-width spaces exactly where shown.

use crate::camera::{CameraId, ClipCategory};
use crate::library::WarningKind;
use crate::timestamp::{LocalTimestamp, Weekday};

// ---------------------------------------------------------------------------------------------
// Names
// ---------------------------------------------------------------------------------------------

/// Window title and wordmark.
pub const APP_NAME: &str = "TeslaCam Viewer";

/// The Japanese camera name.
pub const fn camera_label(camera: CameraId) -> &'static str {
    match camera {
        CameraId::Front => "フロント",
        CameraId::Back => "リア",
        CameraId::LeftPillar => "左ピラー",
        CameraId::LeftRepeater => "左リピーター",
        CameraId::RightPillar => "右ピラー",
        CameraId::RightRepeater => "右リピーター",
    }
}

/// The full category name (accessible names, details, search haystack).
pub const fn category_label(category: ClipCategory) -> &'static str {
    match category {
        ClipCategory::Recent => "最近の走行",
        ClipCategory::Saved => "保存済み",
        ClipCategory::Sentry => "セントリー",
    }
}

/// The short category name shown in the segmented control and chips (`DESIGN.md` §6 Library).
pub const fn category_short_label(category: ClipCategory) -> &'static str {
    match category {
        ClipCategory::Recent => "走行",
        ClipCategory::Saved => "保存",
        ClipCategory::Sentry => "セントリー",
    }
}

/// The segmented control's "all" item.
pub const CATEGORY_ALL: &str = "すべて";
/// Accessible name of the category segmented control (prototype).
pub const CATEGORY_GROUP: &str = "種類";

// ---------------------------------------------------------------------------------------------
// Welcome, scanning, errors
// ---------------------------------------------------------------------------------------------

/// Welcome headline.
pub const WELCOME_TITLE: &str = "TeslaCamの記録を見る";
/// Welcome paragraph (prototype).
pub const WELCOME_DESCRIPTION: &str = "USBドライブの TeslaCam フォルダを選ぶと、セントリーと保存したクリップ、最近の走行を日ごとに並べて、6台のカメラを同時に再生します。";
/// Primary action on the welcome and error screens.
pub const WELCOME_ACTION: &str = "フォルダを選ぶ";
/// The two notes at the bottom of the welcome panel: `(title, body)` (prototype).
pub const WELCOME_NOTES: [(&str, &str); 2] = [
    (
        "セントリーは検知の15秒前から",
        "何が近づいたのかをすぐ確かめられます。",
    ),
    (
        "6カメラを1本の動画に",
        "車両の配置どおりに並べて書き出せます。",
    ),
];
/// Accessible name of the floating library panel (prototype).
pub const LIBRARY_REGION: &str = "ライブラリ";
/// The folder chip's accessible name; it starts a new folder selection.
pub const CHANGE_FOLDER: &str = "別のフォルダを選ぶ";

/// Title of the scanning progress block.
pub const SCAN_TITLE: &str = "読み込み中";
/// Cancels a running scan.
pub const SCAN_CANCEL: &str = "キャンセル";

/// `{n}件を確認しました` with `ja-JP` digit grouping, e.g. `1,250件を確認しました`.
pub fn scan_progress_text(processed: u64) -> String {
    format!("{}件を確認しました", group_digits(processed))
}

/// Why a folder could not be opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FolderError {
    /// Read permission was refused.
    Denied,
    /// No files, or no events (not a TeslaCam folder).
    Invalid,
    /// Any read error, including every scan limit.
    Failed,
}

/// `(title, description)` of an error screen.
pub const fn folder_error_text(error: FolderError) -> (&'static str, &'static str) {
    match error {
        FolderError::Denied => (
            "フォルダへのアクセスが許可されませんでした。",
            "TeslaCamフォルダへの読み取りアクセスを許可して、もう一度選んでください。",
        ),
        FolderError::Invalid => (
            "TeslaCamフォルダを確認できませんでした。",
            "RecentClips、SavedClips、SentryClipsを含むフォルダを選んでください。",
        ),
        FolderError::Failed => (
            "フォルダの読み取りを完了できませんでした。",
            "SSDの接続を確認してから、もう一度選んでください。",
        ),
    }
}

/// Retry button on error screens.
pub const ERROR_RETRY: &str = "もう一度選ぶ";

// ---------------------------------------------------------------------------------------------
// Library: search, day navigation, rail, rows, warnings
// ---------------------------------------------------------------------------------------------

/// Search button / field label.
pub const SEARCH_LABEL: &str = "検索";
/// Search field placeholder.
pub const SEARCH_PLACEHOLDER: &str = "日時またはカテゴリ";
/// Date picker label (prototype).
pub const DATE_LABEL: &str = "日付";
/// Previous recorded day.
pub const PREVIOUS_DAY: &str = "前の記録日";
/// Next recorded day.
pub const NEXT_DAY: &str = "次の記録日";
/// Event list heading (prototype).
pub const EVENT_LIST_TITLE: &str = "イベント";
/// No event matches the filters.
pub const EVENT_LIST_EMPTY: &str = "この条件に一致するイベントはありません。";
/// The shown day has no events.
pub const DAY_EMPTY: &str = "この日のイベントはありません。";
/// Accessible name of the day rail's marker group.
pub const DAY_MARKERS: &str = "この日の記録";

/// The weekday name, e.g. `木曜日`.
pub const fn weekday_label(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Monday => "月曜日",
        Weekday::Tuesday => "火曜日",
        Weekday::Wednesday => "水曜日",
        Weekday::Thursday => "木曜日",
        Weekday::Friday => "金曜日",
        Weekday::Saturday => "土曜日",
        Weekday::Sunday => "日曜日",
    }
}

/// The day title, e.g. `8月20日 木曜日`.
pub fn day_title(date: &LocalTimestamp) -> String {
    format!(
        "{}月{}日 {}",
        date.month,
        date.day,
        weekday_label(date.weekday())
    )
}

/// Accessible name of a day navigation button with a target day, e.g. `前の記録日 2026-08-19`.
pub fn day_button_label(previous: bool, date_key: &str) -> String {
    format!(
        "{} {date_key}",
        if previous { PREVIOUS_DAY } else { NEXT_DAY }
    )
}

/// A cluster pill's range, e.g. `20:35–20:45` (U+2013).
pub fn cluster_range(start_time_label: &str, end_time_label: &str) -> String {
    format!(
        "{}\u{2013}{}",
        hour_minute(start_time_label),
        hour_minute(end_time_label)
    )
}

/// A count with `件`, e.g. `3件` (no digit grouping).
pub fn count_text(count: usize) -> String {
    format!("{count}件")
}

/// Accessible name of a cluster pill, e.g. `20:35–20:45・3件を開く`.
pub fn cluster_toggle_label(
    start_time_label: &str,
    end_time_label: &str,
    count: usize,
    expanded: bool,
) -> String {
    format!(
        "{}\u{30FB}{}{}",
        cluster_range(start_time_label, end_time_label),
        count_text(count),
        if expanded {
            "を閉じる"
        } else {
            "を開く"
        }
    )
}

/// Accessible name of a timeline marker, e.g. `2026-08-24 20:40:00 保存済み`.
pub fn marker_label(date_key: &str, time_label: &str, category: ClipCategory) -> String {
    format!("{date_key} {time_label} {}", category_label(category))
}

fn hour_minute(time_label: &str) -> &str {
    time_label.get(..5).unwrap_or(time_label)
}

/// The number of cameras with footage in every segment, e.g. `6カメラ`.
pub fn camera_count_text(available: usize) -> String {
    format!("{available}カメラ")
}

/// The fault line naming the cameras without footage, e.g. `右リピーターの記録がありません`.
/// `None` when nothing is missing.
pub fn missing_cameras_text(missing: &[CameraId]) -> Option<String> {
    (!missing.is_empty()).then(|| format!("{}の記録がありません", camera_list(missing)))
}

fn camera_list(cameras: &[CameraId]) -> String {
    cameras
        .iter()
        .map(|camera| camera_label(*camera))
        .collect::<Vec<_>>()
        .join("、")
}

/// A detection fact, e.g. `20:35:30に検知`.
pub fn detection_time_text(time_label: &str) -> String {
    format!("{time_label}に検知")
}

/// The detection in the event pill: the reason in words when it is known, e.g.
/// `物体の接近を20:35:30に検知`; otherwise the same as [`detection_time_text`].
pub fn detection_summary(reason: Option<&str>, time_label: &str) -> String {
    match reason.and_then(reason_words) {
        Some(words) => format!("{words}を{}", detection_time_text(time_label)),
        None => detection_time_text(time_label),
    }
}

/// Words for a known `event.json` reason.
pub fn reason_words(reason: &str) -> Option<&'static str> {
    match reason {
        "sentry_aware_object_detection" => Some("物体の接近"),
        _ => None,
    }
}

/// A scan warning, e.g. `対応外のファイル 3 件を除外しました。` (no digit grouping).
pub fn warning_text(kind: WarningKind, count: u64) -> String {
    match kind {
        WarningKind::EmptyMediaFile => format!("空の映像ファイル {count} 件を除外しました。"),
        WarningKind::InvalidMetadataFile => {
            format!("読み取れないイベント情報 {count} 件を除外しました。")
        }
        WarningKind::UnknownFile => format!("対応外のファイル {count} 件を除外しました。"),
        WarningKind::EncryptedFile => {
            format!("暗号化されたファイル {count} 件は読み込みませんでした。")
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Event pill and details popover
// ---------------------------------------------------------------------------------------------

/// Suffix after the start time in the event pill.
pub const EVENT_START_SUFFIX: &str = "開始";
/// Accessible name of the details popover.
pub const DETAILS_REGION: &str = "イベント詳細";
/// Details row: location.
pub const DETAILS_LOCATION: &str = "場所（Teslaの推定値）";
/// Details row: coordinates.
pub const DETAILS_COORDINATES: &str = "推定座標";
/// Details row: detection.
pub const DETAILS_DETECTION: &str = "検知";
/// Details row: where playback started.
pub const DETAILS_PLAYBACK_START: &str = "再生開始";
/// Details row: cameras.
pub const DETAILS_CAMERAS: &str = "カメラ";
/// Details row: duration.
pub const DETAILS_DURATION: &str = "長さ";
/// Details row: segment count.
pub const DETAILS_SEGMENTS: &str = "セグメント";
/// Details row: recording start.
pub const DETAILS_RECORDING_START: &str = "録画開始";
/// Closed disclosure with the raw `reason`.
pub const DETAILS_TECHNICAL: &str = "技術情報";

/// The location value: trimmed city and street joined by one space; `None` when both are empty.
pub fn location_text(city: Option<&str>, street: Option<&str>) -> Option<String> {
    let parts = [city, street]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join(" "))
}

/// The coordinates value `lat, lon`; `None` unless both are non-empty after trimming.
pub fn coordinates_text(latitude: Option<&str>, longitude: Option<&str>) -> Option<String> {
    let latitude = latitude.map(str::trim).filter(|value| !value.is_empty())?;
    let longitude = longitude.map(str::trim).filter(|value| !value.is_empty())?;
    Some(format!("{latitude}, {longitude}"))
}

/// The playback start value, e.g. `2026-08-24 12:01:05（検知の15秒前）`.
pub fn playback_start_text(date_time_label: &str) -> String {
    format!("{date_time_label}（{PRE_ROLL_TICK}）")
}

// ---------------------------------------------------------------------------------------------
// Actions pill, stage, camera windows
// ---------------------------------------------------------------------------------------------

/// Accessible name of the view segmented control (prototype).
pub const VIEW_GROUP: &str = "表示";
/// View: cameras around the car.
pub const VIEW_VEHICLE: &str = "車両配置";
/// View: 3 × 2 grid.
pub const VIEW_GRID: &str = "6分割";
/// View: the single-stream `event.mp4`.
pub const VIEW_REPRESENTATIVE: &str = "代表映像";
/// Accessible name of the stage (prototype).
pub const STAGE_REGION: &str = "6台のカメラ";

/// Accessible name of a camera window that expands on activation.
pub fn camera_expand_label(camera: CameraId) -> String {
    format!("{}カメラを拡大", camera_label(camera))
}

/// Accessible name of the expanded camera window.
pub fn camera_collapse_label(camera: CameraId) -> String {
    format!("{}カメラをグリッドへ戻す", camera_label(camera))
}

/// Camera window: the file is being opened.
pub const TILE_LOADING: &str = "読み込み中";
/// Camera window: no footage for this camera at this time.
pub const TILE_MISSING: &str = "記録がありません";
/// Camera window: the file cannot be read.
pub const TILE_UNAVAILABLE: &str = "映像を読み込めません";
/// Camera window: decode error.
pub const TILE_CORRUPT: &str = "映像が破損しています";
/// Camera window: unsupported format.
pub const TILE_UNSUPPORTED: &str = "未対応の映像形式です";

/// Representative view title.
pub const REPRESENTATIVE_TITLE: &str = "代表映像（単独再生）";
/// Representative view note.
pub const REPRESENTATIVE_NOTE: &str = "6カメラ同期映像ではありません。";
/// `event.mp4` cannot be read.
pub const REPRESENTATIVE_UNAVAILABLE: &str = "代表映像を読み込めません。";

// ---------------------------------------------------------------------------------------------
// Transport
// ---------------------------------------------------------------------------------------------

/// Accessible name of the transport.
pub const TRANSPORT_REGION: &str = "再生コントロール";
/// Previous (older) event.
pub const PREVIOUS_EVENT: &str = "過去の記録";
/// Next (newer) event.
pub const NEXT_EVENT: &str = "新しい記録";
/// Play.
pub const PLAY: &str = "再生";
/// Pause.
pub const PAUSE: &str = "一時停止";
/// Skip back 10 s.
pub const SKIP_BACK: &str = "10秒戻る";
/// Skip forward 10 s.
pub const SKIP_FORWARD: &str = "10秒進む";
/// The rate menu's accessible name.
pub const PLAYBACK_RATE: &str = "再生速度";
/// The scrubber's accessible name.
pub const PLAYBACK_POSITION: &str = "再生位置";
/// The detection tick on the scrubber.
pub const DETECTION_TICK: &str = "検知";
/// The pre-roll start tick on the scrubber.
pub const PRE_ROLL_TICK: &str = "検知の15秒前";

/// A rate value, e.g. `1.5×` (U+00D7).
pub fn rate_label(rate: f64) -> String {
    format!("{rate}\u{00D7}")
}

/// The clock, `M:SS` with unpadded minutes, e.g. `1:05`.
pub fn clock_text(seconds: f64) -> String {
    let whole = whole_seconds(seconds);
    format!("{}:{:02}", whole / 60, whole % 60)
}

/// The duration after the clock, e.g. `/ 1:00`.
pub fn clock_duration_text(seconds: f64) -> String {
    format!("/ {}", clock_text(seconds))
}

/// The scrubber's value text, e.g. `0分15秒 / 1分`.
pub fn position_value_text(position: f64, duration: f64) -> String {
    format!(
        "{} / {}",
        minutes_seconds(position),
        minutes_seconds(duration)
    )
}

fn minutes_seconds(seconds: f64) -> String {
    let whole = whole_seconds(seconds);
    match whole % 60 {
        0 => format!("{}分", whole / 60),
        rest => format!("{}分{rest}秒", whole / 60),
    }
}

fn whole_seconds(seconds: f64) -> u64 {
    if seconds.is_finite() && seconds > 0.0 {
        seconds.floor() as u64
    } else {
        0
    }
}

/// Shown when play is requested and no camera can play (two lines).
pub const PLAYBACK_NO_PLAYABLE: &str =
    "再生できるカメラ映像がありません。\n読み込み後に再試行してください。";
/// Shown when the media layer could not start playback (the browser half of the web text is dropped).
pub const PLAYBACK_REJECTED: &str = "映像を再生できませんでした。";

// ---------------------------------------------------------------------------------------------
// Composite export
// ---------------------------------------------------------------------------------------------

/// The export action.
pub const EXPORT: &str = "書き出す";
/// Dialog title and the export action's accessible name.
pub const EXPORT_DIALOG_TITLE: &str = "合成動画を書き出す";
/// Dialog close button's accessible name.
pub const EXPORT_DIALOG_CLOSE: &str = "書き出し画面を閉じる";
/// Dialog paragraph.
pub const EXPORT_DIALOG_DESCRIPTION: &str = "選択中の記録を、6カメラと車両配置を含む1本の動画にまとめます。利用できないカメラは「記録なし」と表示します。";
/// Dialog row: the event.
pub const EXPORT_RECORD: &str = "記録";
/// Dialog row: range.
pub const EXPORT_RANGE: &str = "範囲";
/// Dialog row: resolution.
pub const EXPORT_RESOLUTION: &str = "解像度";
/// Dialog row: format.
pub const EXPORT_FORMAT: &str = "形式";
/// Resolution value (U+00D7).
pub const EXPORT_RESOLUTION_VALUE: &str = "1280 \u{00D7} 720";
/// Format value on the Mac.
pub const EXPORT_FORMAT_VALUE: &str = "MP4";
/// Cancel in the dialog.
pub const EXPORT_CANCEL: &str = "キャンセル";
/// Start in the dialog.
pub const EXPORT_START: &str = "書き出しを開始";
/// Aborts a running export.
pub const EXPORT_ABORT: &str = "中止";
/// Shown after an abort.
pub const EXPORT_ABORTED: &str = "書き出しを中止しました。";
/// Completion status.
pub const EXPORT_DONE: &str = "書き出し完了";
/// Closes the completion state.
pub const EXPORT_CLOSE: &str = "閉じる";
/// Retries after an error.
pub const EXPORT_RETRY: &str = "もう一度";
/// The export is unavailable because no camera has footage in every segment.
pub const EXPORT_UNAVAILABLE: &str = "6カメラ映像がないため書き出せません";
/// Fallback error message.
pub const EXPORT_ERROR_GENERIC: &str = "合成動画を書き出せませんでした。もう一度お試しください。";
/// The car image could not be loaded.
pub const EXPORT_ERROR_VEHICLE: &str = "車両イメージを読み込めませんでした。";
/// Encoder or writer failure.
pub const EXPORT_ERROR_RECORDING: &str = "動画の録画に失敗しました。";
/// The event produced no frames.
pub const EXPORT_ERROR_NO_MEDIA: &str = "再生可能な動画がありません。";
/// Abort reason (never displayed).
pub const EXPORT_ERROR_ABORTED: &str = "動画の書き出しをキャンセルしました。";
/// Written into the video for a camera without footage.
pub const EXPORT_MISSING_CAMERA: &str = "記録なし";

/// The range value, e.g. `イベント全体・1分` (U+30FB).
pub fn export_range_text(duration_seconds: u64) -> String {
    format!("イベント全体\u{30FB}{}", format_duration(duration_seconds))
}

/// The running export, e.g. `書き出し中 42%` (floor of the percentage).
pub fn export_running_text(progress: f64) -> String {
    format!("書き出し中 {}%", progress_percent(progress))
}

/// The floor of `progress × 100`, clamped to 0–100.
pub fn progress_percent(progress: f64) -> u8 {
    if progress.is_nan() {
        return 0;
    }
    (progress.clamp(0.0, 1.0) * 100.0).floor() as u8
}

/// The save action, e.g. `MP4を保存`.
pub fn export_save_label(extension: &str) -> String {
    format!("{}を保存", extension.to_uppercase())
}

/// The exported file's default name, e.g. `TeslaCam-2026-08-20_20-45-00-vehicle.mp4`.
pub fn export_file_name(event_timestamp: &str) -> String {
    format!("TeslaCam-{event_timestamp}-vehicle.mp4")
}

/// The progress clock, `MM:SS / MM:SS` with zero-padded minutes, e.g. `00:30 / 01:00`.
pub fn export_progress_clock(progress: f64, duration_seconds: f64) -> String {
    let clamped = if progress.is_nan() {
        0.0
    } else {
        progress.clamp(0.0, 1.0)
    };
    let padded = |seconds: f64| {
        let whole = whole_seconds(seconds);
        format!("{:02}:{:02}", whole / 60, whole % 60)
    };
    format!(
        "{} / {}",
        padded(clamped * duration_seconds),
        padded(duration_seconds)
    )
}

// ---------------------------------------------------------------------------------------------
// Formatters shared with the web (copy-format.json)
// ---------------------------------------------------------------------------------------------

/// `{s}秒` below a minute, else `{m}分`, `{h}時間` or `{h}時間{m}分` (seconds are dropped).
pub fn format_duration(seconds: u64) -> String {
    if seconds < 60 {
        return format!("{seconds}秒");
    }
    let hours = seconds / 3_600;
    let minutes = seconds % 3_600 / 60;
    match (hours, minutes) {
        (0, minutes) => format!("{minutes}分"),
        (hours, 0) => format!("{hours}時間"),
        (hours, minutes) => format!("{hours}時間{minutes}分"),
    }
}

/// `{available}/{total} カメラ` (web layout; the garage floor shows [`camera_count_text`]).
pub fn camera_availability_label(available: usize, total: usize) -> String {
    format!("{available}/{total} カメラ")
}

/// `{labels joined by 、}なし` (web layout; the garage floor shows [`missing_cameras_text`]).
pub fn missing_camera_label(missing: &[CameraId]) -> Option<String> {
    (!missing.is_empty()).then(|| format!("{}なし", camera_list(missing)))
}

/// `{n}セグメント`.
pub fn segment_count_label(count: usize) -> String {
    format!("{count}セグメント")
}

/// `ja-JP` digit grouping, e.g. `12,345`.
pub fn group_digits(value: u64) -> String {
    let digits = value.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(digit);
    }
    grouped
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::*;
    use crate::contract::{camera_input, category_input, option_json, run_fixture};
    use crate::filters::event_time;
    use crate::timestamp::parse_filename_local_timestamp;

    fn cameras(value: &Value) -> Vec<CameraId> {
        value
            .as_array()
            .expect("cameras")
            .iter()
            .map(camera_input)
            .collect()
    }

    #[test]
    fn matches_the_copy_format_fixture() {
        run_fixture("copy-format", |case| {
            Some(match case.function.as_str() {
                "cameraLabel" => camera_label(camera_input(case.input("camera"))).into(),
                "categoryLabel" => category_label(category_input(case.input("category"))).into(),
                "formatDuration" => {
                    format_duration(case.input("seconds").as_u64().expect("seconds")).into()
                }
                "cameraAvailabilityLabel" => {
                    let missing = cameras(case.input("missing"));
                    camera_availability_label(
                        CameraId::ALL.len() - missing.len(),
                        CameraId::ALL.len(),
                    )
                    .into()
                }
                "missingCameraLabel" => {
                    option_json(missing_camera_label(&cameras(case.input("missing"))))
                }
                "segmentCountLabel" => segment_count_label(
                    case.input("segmentCount").as_u64().expect("count") as usize,
                )
                .into(),
                "eventTime" => {
                    event_time(case.input("timestamp").as_str().expect("timestamp")).into()
                }
                _ => return None,
            })
        });
    }

    #[test]
    fn scan_progress_groups_digits() {
        assert_eq!(scan_progress_text(0), "0件を確認しました");
        assert_eq!(scan_progress_text(1_250), "1,250件を確認しました");
        assert_eq!(scan_progress_text(1_234_567), "1,234,567件を確認しました");
        assert_eq!(group_digits(999), "999");
        assert_eq!(group_digits(100_000), "100,000");
    }

    #[test]
    fn warnings_do_not_group_digits() {
        assert_eq!(
            warning_text(WarningKind::UnknownFile, 1_234),
            "対応外のファイル 1234 件を除外しました。"
        );
        assert_eq!(
            warning_text(WarningKind::EncryptedFile, 3),
            "暗号化されたファイル 3 件は読み込みませんでした。"
        );
        assert_eq!(
            warning_text(WarningKind::EmptyMediaFile, 1),
            "空の映像ファイル 1 件を除外しました。"
        );
        assert_eq!(
            warning_text(WarningKind::InvalidMetadataFile, 2),
            "読み取れないイベント情報 2 件を除外しました。"
        );
    }

    #[test]
    fn day_title_names_the_weekday() {
        let date = parse_filename_local_timestamp("2026-08-20_00-00-00").expect("valid");
        assert_eq!(day_title(&date), "8月20日 木曜日");
        assert_eq!(
            day_button_label(true, "2026-08-19"),
            "前の記録日 2026-08-19"
        );
        assert_eq!(
            day_button_label(false, "2026-08-21"),
            "次の記録日 2026-08-21"
        );
    }

    #[test]
    fn cluster_copy_uses_the_en_dash_and_middle_dot() {
        assert_eq!(cluster_range("20:35:00", "20:45:10"), "20:35\u{2013}20:45");
        assert_eq!(
            cluster_toggle_label("12:00:00", "13:00:00", 2, false),
            "12:00\u{2013}13:00\u{30FB}2件を開く"
        );
        assert_eq!(
            cluster_toggle_label("12:00:00", "13:00:00", 2, true),
            "12:00\u{2013}13:00\u{30FB}2件を閉じる"
        );
        assert_eq!(
            marker_label("2026-08-24", "20:40:00", ClipCategory::Saved),
            "2026-08-24 20:40:00 保存済み"
        );
    }

    #[test]
    fn event_facts_follow_the_garage_floor_copy() {
        assert_eq!(camera_count_text(6), "6カメラ");
        assert_eq!(
            missing_cameras_text(&[CameraId::RightRepeater]).as_deref(),
            Some("右リピーターの記録がありません")
        );
        assert_eq!(
            missing_cameras_text(&[CameraId::Back, CameraId::LeftPillar]).as_deref(),
            Some("リア、左ピラーの記録がありません")
        );
        assert_eq!(missing_cameras_text(&[]), None);
        assert_eq!(detection_time_text("20:35:30"), "20:35:30に検知");
        assert_eq!(
            detection_summary(Some("sentry_aware_object_detection"), "20:35:30"),
            "物体の接近を20:35:30に検知"
        );
        assert_eq!(
            detection_summary(Some("user_interaction_honk"), "20:35:30"),
            "20:35:30に検知"
        );
        assert_eq!(detection_summary(None, "20:35:30"), "20:35:30に検知");
        assert_eq!(category_short_label(ClipCategory::Recent), "走行");
        assert_eq!(category_short_label(ClipCategory::Saved), "保存");
    }

    #[test]
    fn details_values() {
        assert_eq!(
            location_text(Some(" Sample city "), Some("Sample street")).as_deref(),
            Some("Sample city Sample street")
        );
        assert_eq!(
            location_text(None, Some("Sample street")).as_deref(),
            Some("Sample street")
        );
        assert_eq!(location_text(Some(" "), Some("")), None);
        assert_eq!(
            coordinates_text(Some("35.0000"), Some(" 139.0000 ")).as_deref(),
            Some("35.0000, 139.0000")
        );
        assert_eq!(coordinates_text(Some("35.0000"), None), None);
        assert_eq!(
            playback_start_text("2026-08-24 12:01:05"),
            "2026-08-24 12:01:05（検知の15秒前）"
        );
    }

    #[test]
    fn transport_copy() {
        assert_eq!(clock_text(15.4), "0:15");
        assert_eq!(clock_text(65.0), "1:05");
        assert_eq!(clock_text(300.0), "5:00");
        assert_eq!(clock_text(f64::NAN), "0:00");
        assert_eq!(clock_duration_text(60.0), "/ 1:00");
        assert_eq!(position_value_text(15.0, 60.0), "0分15秒 / 1分");
        assert_eq!(position_value_text(0.0, 125.0), "0分 / 2分5秒");
        assert_eq!(rate_label(0.5), "0.5\u{00D7}");
        assert_eq!(rate_label(1.0), "1\u{00D7}");
        assert_eq!(rate_label(1.5), "1.5\u{00D7}");
        assert_eq!(rate_label(2.0), "2\u{00D7}");
        assert_eq!(PLAYBACK_NO_PLAYABLE.lines().count(), 2);
    }

    #[test]
    fn export_copy() {
        assert_eq!(export_running_text(0.42), "書き出し中 42%");
        assert_eq!(export_running_text(0.999), "書き出し中 99%");
        assert_eq!(export_running_text(1.5), "書き出し中 100%");
        assert_eq!(export_save_label("mp4"), "MP4を保存");
        assert_eq!(
            export_file_name("2026-08-20_20-45-00"),
            "TeslaCam-2026-08-20_20-45-00-vehicle.mp4"
        );
        assert_eq!(export_progress_clock(0.5, 60.0), "00:30 / 01:00");
        assert_eq!(export_progress_clock(1.0, 3_600.0), "60:00 / 60:00");
        assert_eq!(export_range_text(60), "イベント全体\u{30FB}1分");
        assert_eq!(EXPORT_RESOLUTION_VALUE, "1280 \u{00D7} 720");
    }

    #[test]
    fn error_screens() {
        assert_eq!(
            folder_error_text(FolderError::Invalid).0,
            "TeslaCamフォルダを確認できませんでした。"
        );
        assert_eq!(
            folder_error_text(FolderError::Failed).1,
            "SSDの接続を確認してから、もう一度選んでください。"
        );
    }

    #[test]
    fn no_copy_uses_the_forbidden_meta_separator() {
        let strings = [
            WELCOME_TITLE,
            WELCOME_DESCRIPTION,
            SCAN_TITLE,
            EVENT_LIST_EMPTY,
            DAY_EMPTY,
            EXPORT_DIALOG_DESCRIPTION,
            EXPORT_DONE,
            PLAYBACK_NO_PLAYABLE,
            PLAYBACK_REJECTED,
        ];
        for text in strings {
            assert!(!text.contains('\u{00B7}'), "{text}");
            assert!(!text.contains("ブラウザ"), "{text}");
            assert!(!text.contains("台のカメラがありません"), "{text}");
        }
    }
}
