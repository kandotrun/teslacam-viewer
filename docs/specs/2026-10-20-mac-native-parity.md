# TeslaCam Viewer — macOS native parity specification

| Field | Value |
|---|---|
| Status | Draft for implementation and review |
| Date | 2026-10-20 |
| Reference implementation | `apps/web` at commit `3fe4d93` (`fix(copy): ローカル処理表記を公開メタ情報から除く`) |
| Target | `apps/mac` (Rust, GPUI), feature parity with the web app |
| Shared contract | `packages/contract/` (proposed in §6) |

This document is the contract that the macOS app and its reviewers check against. It was derived by reading the
entire web app (`src/**`, `tests/e2e/**`, `DESIGN.md`, `docs/superpowers/specs/*.md`, `public/*`, `index.html`).
Where the older design documents and the code disagree, **the code at `3fe4d93` wins**; such conflicts are listed in §8.

## 0. Conventions

- **MUST / SHOULD / MAY** are used as in RFC 2119. Anything not marked otherwise in §1 and §3 is MUST.
- Japanese UI strings are quoted verbatim in backticks. `{name}` marks a placeholder. Every user-visible string the
  Mac app renders MUST come from the catalog in §5 (and, once it exists, `packages/contract/strings/ja.json`).
- "Web" means the reference implementation; "Mac" means `apps/mac`.
- File references such as `event-detection.ts` are relative to `apps/web/src/react-app/` or `apps/web/src/teslacam/`.
- Pixel values are CSS pixels on the web and **points** on the Mac (1 CSS px = 1 pt).
- Timestamps are **local wall-clock strings**. Neither app converts them through a time zone, `Date`, UTC, or the
  system calendar. All time arithmetic is done on calendar fields (§1.12).
- Section tags such as **[N/A on Mac]** mark web-only behavior that the Mac app does not implement; they are listed so
  reviewers can confirm the omission is deliberate.

### 0.1 Glossary

| Term | Meaning |
|---|---|
| Clip | One camera MP4 of roughly one minute, e.g. `2026-08-21_14-30-45-front.mp4` |
| Segment | All clips that share one filename timestamp inside one event |
| Event | An ordered list of segments plus optional helper files (§1.6, §1.7) |
| Directory event | A Saved or Sentry event, one per event directory |
| Recent session | A Recent event built by joining Recent segments (§1.7.2) |
| Library | All events, events per category, and scan warnings |
| Theater session | Everything owned by the currently selected event: playback runtime, camera tile states, expanded camera, active camera, media handles, export job. It is recreated whenever the selected event id changes |
| Elapsed seconds | Shared playback position inside an event on the nominal 60 s/segment axis (§1.17) |

---

## 1. Domain rules

### 1.1 Cameras

Canonical camera ids and order (`types.ts` `cameraIds`). This order is used for DOM/tile order, readiness scanning,
source election, the digit shortcuts, camera availability, and missing-camera labels.

| # | Id | Label (`cameraLabels`) | Digit key |
|---|---|---|---|
| 1 | `front` | `フロント` | `1` |
| 2 | `back` | `リア` | `2` |
| 3 | `left_pillar` | `左ピラー` | `3` |
| 4 | `left_repeater` | `左リピーター` | `4` |
| 5 | `right_pillar` | `右ピラー` | `5` |
| 6 | `right_repeater` | `右リピーター` | `6` |

Other orders that exist and MUST be reproduced exactly:

- **Composite slot order** (draw order in the exported video, §3.2): `front`, `left_pillar`, `right_pillar`,
  `left_repeater`, `back`, `right_repeater`.
- **Spatial stage areas** (§2.10.3): row 1 `· front ·`; row 2 `left_pillar vehicle right_pillar`;
  row 3 `left_repeater back right_repeater`.
- **Grid stage** (§2.10.3): canonical order filling 3 columns × 2 rows: row 1 `front back left_pillar`,
  row 2 `left_repeater right_pillar right_repeater`.

Clip categories: `recent`, `saved`, `sentry`. Category labels (`categoryLabel`): `recent` → `最近の走行`,
`saved` → `保存済み`, `sentry` → `セントリー`.

### 1.2 TeslaCam folder structure (input)

```text
TeslaCam/                         ← the user usually selects this folder (any folder works, see §1.5)
├── RecentClips/                  ← clips directly inside (sub-directories are tolerated)
│   └── 2026-08-21_14-30-45-front.mp4 …
├── SavedClips/
│   └── <event directory>/        ← one directory = one event
│       ├── 2026-08-21_14-30-45-<camera>.mp4 …
│       ├── event.mp4             ← representative video (optional)
│       ├── event.json            ← metadata (optional, §1.11)
│       └── thumb.png             ← thumbnail (optional)
├── SentryClips/
│   └── <event directory>/        ← same layout as SavedClips
└── EncryptedClips/               ← counted, never played
```

Real footage is H.264, 1448×938, 24 fps, about 60 s per clip; any camera may be missing from any segment, and that is
a normal state, not an error.

### 1.3 Folder ingestion (traversal)

Web: `directory-picker.ts` (`showDirectoryPicker`) or `directory-input.ts` (`<input webkitdirectory>` fallback,
**[N/A on Mac]**). Mac: the user picks one directory with a native open panel (directories only, single selection)
and the app walks it. The walk MUST reproduce `pickTeslaCamDirectory`:

1. `rootName` = the display name of the selected directory (its last path component; for a volume root, the volume
   name). `folderName` = `rootName`.
2. Walk depth-first. The root directory has depth 0; a directory found inside a directory of depth *d* has depth
   *d* + 1. **On entering** a directory whose depth is greater than `MAX_DIRECTORY_DEPTH` (8) the scan fails with
   `ScanLimitError('directory-depth')`, even if that directory is empty.
3. For **every** entry enumerated in a directory (files, directories, hidden entries, anything else) increment
   `processed` first. If `processed > MAX_SCAN_ENTRIES` (100 000) fail with `ScanLimitError('entry-count')`.
   If `processed % 250 == 0`, report progress `{processed, total: processed}` and yield to the UI.
4. Entries whose name starts with `.` are skipped after being counted: hidden files are not collected and hidden
   directories are not entered (this also drops macOS `._*` AppleDouble files and `.DS_Store`).
5. A regular file becomes a `LocalFileRef`: `name` = entry name, `relativePath` = `[rootName, …parent directory names,
   name].join('/')`, `size` = byte size read at scan time. File contents are **not** read during the walk.
6. A directory is walked recursively (step 2).
7. **Mac decision (no web equivalent):** symbolic links and other non-regular entries are counted (step 3) but neither
   followed nor collected. See §8 A-18.
8. After the walk, report `{processed, total: processed}` once more.
9. Enumeration order on the web is engine-defined. The Mac SHOULD enumerate each directory sorted by entry name
   (byte-wise ascending) so results are deterministic. Order is observable only through ties (§1.8) and duplicate
   clips (§1.7.1); contract fixtures make the order explicit.

Result mapping (web `DirectorySelectionResult` → app action, `use-directory-scan.ts`):

| Situation | Web | Mac | App action |
|---|---|---|---|
| User dismisses the picker | `AbortError` → `cancelled` | open panel cancelled | `scan-cancelled` |
| Read permission refused for the chosen root | `NotAllowedError` → `denied` | `EACCES`/`EPERM` (sandbox/TCC) reading the root | `scan-denied` |
| Picker API missing | `unsupported` → opens fallback input | **[N/A on Mac]** | — |
| Any other error during picking or walking, including every `ScanLimitError` | thrown → caught | any other I/O error | `scan-failed` |
| Walk finished with 0 files | `files.length === 0` | same | `scan-invalid` |
| Library built with 0 events | `library.events.length === 0` | same | `scan-invalid` |
| Library built with ≥ 1 event | — | — | `library-ready {folderName, library}` |
| Any error while building the library (including `ScanLimitError`) | caught | same | `scan-failed` |

Scan generations: every new folder request increments a generation counter; progress, completion and failure from an
older generation are ignored (`use-directory-scan.test.tsx`). The Mac MUST guarantee the same: a stale background scan
can never overwrite the state produced by a newer request.

Fallback-only path rules **[N/A on Mac, listed for completeness]** (`directory-input.ts`): `\` is normalized to `/`;
paths that are absolute, start with a drive letter, or contain an empty, `.`, `..`, dot-prefixed, or `X:` component
are dropped silently; `parts.length - 2 > 8` fails with `directory-depth`; more than 100 000 files fails with
`entry-count` before iterating; `folderName` = first path component of the first file, or `TeslaCam` if there are no
files. The fallback input's value is cleared before each open.

### 1.4 Clip filename grammar (`filename.ts`)

```text
clip      = timestamp "-" camera ".mp4"
timestamp = YYYY "-" MM "-" DD "_" hh "-" mm "-" ss     ; exactly 4/2/2/2/2/2 ASCII digits
camera    = one of the six ids in §1.1                    ; matched against [a-z_]+ first
```

- Regex (JS, `u` flag): `^(\d{4}-\d{2}-\d{2}_\d{2}-\d{2}-\d{2})-([a-z_]+)\.mp4$`. In JS `\d` with the `u` flag is
  ASCII-only. **Rust MUST use `[0-9]`** (Rust's `\d` is Unicode-aware) and MUST match case-sensitively (`.MP4`, `Front`
  do not match).
- Calendar validation: `year > 0`, `1 ≤ month ≤ 12`, `1 ≤ day ≤ daysInMonth(year, month)` with Gregorian leap years
  (`y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)`), `hour ≤ 23`, `minute ≤ 59`, `second ≤ 59`.
- Result: `{timestamp, camera}` or null. `event.mp4`, `thumb.png`, `…-cabin.mp4`, `2026-02-29_…`, `…_24-30-45-…` → null.

### 1.5 Category detection and file classification (`build-library.ts`)

For each collected file *f* (in collection order) with `name`, `relativePath`, `size`:

1. `parts` = `relativePath` split on `/` or `\`. Scan `parts` left to right (the file name itself included) and take
   the **first** part equal to `RecentClips`, `SavedClips`, `SentryClips`, or `EncryptedClips`. The selected root may be
   `TeslaCam`, one category folder, or any ancestor; only the path parts matter.
2. No category part → warning: `empty-media-file` if `name` ends with `.mp4` (case-sensitive) and `size == 0`,
   otherwise `unknown-file`.
3. `EncryptedClips` → warning `encrypted-file` (regardless of name or size).
4. Otherwise the category is `recent`, `saved` or `sentry`. If `name` ends with `.mp4` and `size == 0` → warning
   `empty-media-file` (this includes a 0-byte `event.mp4`).
5. Else, if the category is not `recent` and `name` is exactly `event.mp4`, `event.json` or `thumb.png`: a helper
   file (§1.6) for directory key `"{category}:{parts[i+1]}"` where *i* is the first index of `SavedClips`/`SentryClips`
   in `parts`. If `parts[i+1]` does not exist → `unknown-file`.
6. Else parse the name with §1.4. Null → `unknown-file`.
7. `recent`: `recentClips[timestamp][camera] = f` (a later file with the same timestamp and camera overwrites).
8. `saved`/`sentry`: directory key as in step 5 (missing → `unknown-file`);
   `directoryEvent[key].clips[timestamp][camera] = f` (overwrite on duplicates).

Consequences that MUST be preserved (see §8 A-19, A-20):

- Helper names inside `RecentClips` are not helpers; they become `unknown-file`.
- A clip placed directly in `SavedClips/` (no event directory) gets the key `saved:<its own file name>`, so it forms a
  one-file event.
- Clips in sub-directories of `RecentClips` are accepted. For Saved/Sentry only the first directory below the category
  folder defines the event.

Progress while classifying: after file *k* (1-based), if `k % 250 == 0` report `{processed: k, total: N}` and yield;
after the loop, if `N % 250 != 0` report `{processed: N, total: N}`.

### 1.6 Helper files

| Name | Role | Rule |
|---|---|---|
| `event.mp4` | `representativeFile` | Single-stream video shown only in representative mode (§2.10.7). Never part of 6-camera sync or export |
| `event.json` | `metadataFile` | Read once after classification, only if the event has ≥ 1 segment (§1.11) |
| `thumb.png` | `thumbnailFile` | Shown in the event row if it loads (§2.8); nothing is shown otherwise |

A later helper with the same name in the same directory key replaces an earlier one. A 0-byte `thumb.png` or
`event.json` is still registered (only `.mp4` files are size-checked).

### 1.7 Event grouping

#### 1.7.1 Directory events (Saved, Sentry)

Iterate directory events in **first-seen order** of their key (the order in which the first file of that directory,
helper or clip, was classified). For each:

1. `segments` = its clip timestamps sorted ascending (by ordinal seconds, §1.12), each `{timestamp, cameras}` where
   `cameras` is a partial map camera → file.
2. If there are no segments, drop the event: its metadata is not read and no warning is added for it.
3. If an `event.json` exists, read it (§1.11). A scan-limit error aborts the whole scan. A null result adds one
   `invalid-metadata-file` warning and the event has no metadata.
4. `timestamp` = the first segment's timestamp (**not** the `event.json` timestamp).
5. Event = `{category, timestamp, segments, representativeFile?, thumbnailFile?, metadata?}`.

Directory events are never split by time gaps, however far apart their segments are.

#### 1.7.2 Recent sessions

1. Sort all Recent segments ascending.
2. Walk them, keeping a current session. Start a new session when
   `ordinalSeconds(segment) - ordinalSeconds(previous segment) > 90`. A gap of exactly 90 s joins.
3. Each session becomes an event with `timestamp` = its first segment's timestamp. Sessions may cross midnight and
   years; the event belongs to its first segment's date everywhere (list, filters, timeline).
4. Recent events never have helper files or metadata.

Example (`build-library.test.ts`): `23:59:30`, `00:00:50` (+80 s, joined), `00:02:21` (+91 s, split) → two events,
newest first: `2026-08-22_00-02-21` (1 segment), `2026-08-21_23-59-30` (2 segments).

### 1.8 Sorting and ids

1. Per category, sort events by timestamp **descending**, stable (ties keep first-seen order).
2. Assign `id = "{category}-{timestamp}-{rank}"`, rank 1-based within the category after sorting.
   Example: `sentry-2026-08-21_14-30-45-1`, `sentry-2026-08-21_14-30-45-2` for two Sentry directories with equal start.
3. `events` = concatenation `recent ++ saved ++ sentry`, then stable sort by timestamp descending. Ties across categories
   therefore order Recent, Saved, Sentry.
4. `eventsByCategory` keeps the per-category lists from step 1.

Timestamp comparison MUST use ordinal seconds (equivalently byte-wise comparison of the fixed-width timestamp
strings). The web's `localeCompare` on these strings gives the same order.

### 1.9 Scan limits (`scan-limits.ts`)

| Constant | Value | Checked where | Error kind |
|---|---|---|---|
| `MAX_SCAN_ENTRIES` | `100_000` | every enumerated entry during the walk (> fails); file count before classifying (> fails) | `entry-count` |
| `MAX_DIRECTORY_DEPTH` | `8` | on entering a directory (depth > 8 fails) | `directory-depth` |
| `MAX_EVENT_METADATA_SIZE` | `1_048_576` bytes | declared file size, then actual read size, of each `event.json` (> fails) | `metadata-size` |
| `MAX_METADATA_STRING_LENGTH` | `2048` | each of the seven known `event.json` fields; length in **UTF-16 code units** (> fails) | `metadata-string` |

Every scan-limit error aborts the whole scan and is shown as the `failed` error state (§2.4). There is no dedicated
scan-limit copy. Error message (developer-facing, never shown): `Selected folder exceeds a safe local scan limit`.

### 1.10 Scan warnings

Warnings carry only a kind and a count (no paths, names or timestamps). They are listed in this fixed order and only
when the count is > 0:

| Order | Kind | Copy (`LibrarySidebar.tsx` `warningCopy`) |
|---|---|---|
| 1 | `empty-media-file` | `空の映像ファイル {count} 件を除外しました。` |
| 2 | `invalid-metadata-file` | `読み取れないイベント情報 {count} 件を除外しました。` |
| 3 | `unknown-file` | `対応外のファイル {count} 件を除外しました。` |
| 4 | `encrypted-file` | `暗号化されたファイル {count} 件は読み込みませんでした。` |

`{count}` is the plain decimal integer (no digit grouping). Warnings never block `library-ready`.

### 1.11 `event.json` schema and reading (`event-metadata.ts`)

```text
EventMetadata = {
  timestamp: string            // required; detection time, format checked later (§1.12)
  city?:      string
  street?:    string
  est_lat?:   string
  est_lon?:   string
  reason?:    string           // e.g. "sentry_aware_object_detection", "manual_save"
  camera?:    string           // stored but never displayed
}
```

Reading algorithm (`readEventMetadata`):

1. If the file's scanned size > 1 048 576 → `ScanLimitError('metadata-size')`.
2. Read the bytes. If the bytes read > 1 048 576 → `ScanLimitError('metadata-size')`.
3. Decode as UTF-8 the way `Blob.text()` does: strip one leading UTF-8 BOM, replace invalid sequences with U+FFFD.
4. Parse JSON (duplicate keys: last wins). Parse failure or I/O failure → null.
5. Validate (`parseEventMetadata`):
   - The value must be a JSON object (not an array, not null, not a primitive), else null.
   - If **any** of the seven known fields is a string longer than 2048 UTF-16 code units → `ScanLimitError('metadata-string')`.
     This is checked even when other fields are invalid (the validator reports all issues; any "too big" issue wins).
   - `timestamp` must be present and a string; each optional known field, if present, must be a string
     (JSON `null` is invalid). Otherwise null.
   - Unknown fields are ignored whatever their type or size.
6. Return only the known fields that are present (absent optionals stay absent; empty strings are kept).

### 1.12 Local timestamps (`local-timestamp.ts`)

`LocalTimestamp = {year, month, day, hour, minute, second, millisecond, dateKey, timeLabel, secondsOfDay, ordinalSeconds}`

| Function | Accepted grammar | Notes |
|---|---|---|
| `parseFilenameLocalTimestamp` | `^(\d{4})-(\d{2})-(\d{2})_(\d{2})-(\d{2})-(\d{2})$` | `millisecond = 0` |
| `parseMetadataLocalTimestamp` | `^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.(\d{1,3}))?Z?$` | fraction right-padded to 3 digits (`.25` → 250, `.2` → 200); a trailing `Z` is accepted and ignored (no UTC conversion); `+09:00`, zone names, `_` separators are rejected |

Validation (both): `year ≥ 1`, `1 ≤ month ≤ 12`, `1 ≤ day ≤ daysInMonth`, `0 ≤ hour < 24`, `0 ≤ minute < 60`,
`0 ≤ second < 60`, `0 ≤ millisecond < 1000`; otherwise null.

Derived fields:

- `dateKey = "YYYY-MM-DD"` (zero-padded 4/2/2), `timeLabel = "hh:mm:ss"`.
- `secondsOfDay = hour·3600 + minute·60 + second`.
- `daysBeforeYear(y) = (y−1)·365 + ⌊(y−1)/4⌋ − ⌊(y−1)/100⌋ + ⌊(y−1)/400⌋`.
- `ordinalSeconds = (daysBeforeYear(year) + daysBeforeMonth(year, month) + day − 1) · 86 400 + secondsOfDay`
  (milliseconds are **not** included). `0001-01-01_00-00-00` → 0; `2026-08-24_23-59-59` → 63 923 212 799.

`formatFilenameTimestampAtOffset(value, offsetSeconds)`: null if `offsetSeconds` is not an integer or `value` is not
a valid filename timestamp; otherwise convert `ordinalSeconds + offset` back to calendar fields (null if negative) and
return `"YYYY-MM-DD hh:mm:ss"`. Examples: (`2026-12-31_23-59-45`, 30) → `2027-01-01 00:00:15`;
(`2028-02-28_23-59-59`, 1) → `2028-02-29 00:00:00`; (`2026-01-01_00-00-10`, −20) → `2025-12-31 23:59:50`.

### 1.13 Event text formatting (`event-filters.ts`)

| Function | Rule | Example (`2026-08-20_20-45-00`) |
|---|---|---|
| `eventDate` | first 10 characters | `2026-08-20` |
| `eventClock` | characters from index 11, `-` → `:` | `20:45:00` |
| `eventTime` | `"{eventDate} {eventClock}"` | `2026-08-20 20:45:00` |
| `categoryLabel` | §1.1 | `セントリー` |

These operate on the event's start timestamp string directly.

### 1.14 Event summaries (`event-summary.ts`, `playback/timeline.ts`)

- `eventDuration(event) = segments.length × 60` seconds (nominal; actual clip lengths are ignored).
- `formatDuration(s)`: `s < 60` → `{s}秒`; else `h = ⌊s/3600⌋`, `m = ⌊(s mod 3600)/60⌋`; `h == 0` → `{m}分`;
  `m == 0` → `{h}時間`; else `{h}時間{m}分`. Examples: 60 → `1分`, 660 → `11分`, 3600 → `1時間`, 3660 → `1時間1分`.
- `segmentCountLabel` → `{n}セグメント`.
- `cameraAvailability(event)`: a camera is **missing** if the event has no segments or **any** segment lacks it;
  `available = 6 − |missing|`, `total = 6`, `missing` in canonical order.
- `cameraAvailabilityLabel` → `{available}/{total} カメラ` (half-width space before `カメラ`), e.g. `6/6 カメラ`, `1/6 カメラ`.
- `missingCameraLabel` → null when nothing is missing, else labels joined with `、` (U+3001) followed by `なし`:
  `右リピーターなし`; `リア、左ピラー、左リピーター、右ピラー、右リピーターなし`.

Copy rule from DESIGN.md: the healthy state is stated positively (`6/6 カメラ`); never use `0台のカメラがありません`.

### 1.15 Filters (`event-filters.ts`, `app-state.ts`)

`EventFilters = {category: 'all' | ClipCategory, dateKey: string | null, query: string}`; initial
`{category: 'all', dateKey: null, query: ''}`.

`eventMatchesFilters(event, filters)`:

1. `category != 'all'` and `event.category != category` → false.
2. `dateKey != null` and `event.timestamp` does not start with `dateKey` (raw string prefix) → false.
3. `q = query.trim().toLowerCase()`; true if `q == ''` or the haystack
   `"{eventTime} {category id} {categoryLabel}"` lower-cased contains `q`
   (e.g. `2026-08-20 20:45:00 sentry セントリー`). `trim` removes Unicode white space at both ends.

`orderedMatchingEvents(events, filters)` = matching events stably sorted by timestamp descending.

Workspace uses two derived lists (`ReadyWorkspace.tsx`):

- `matchingEvents` = `orderedMatchingEvents(library.events, filters)` — event list, previous/next navigation.
- `timelineEvents` = `orderedMatchingEvents(library.events, {...filters, dateKey: null})` — day timeline input
  (category and query apply, the date filter does not; see §8 A-15).

Changing any filter clears `selectedEventId` if the selected event no longer matches the new filters.

### 1.16 Sentry detection and 15 s pre-roll (`event-detection.ts`)

`detectionSeconds(event)` (any category):

1. `d = parseMetadataLocalTimestamp(event.metadata?.timestamp ?? '')`; null → null.
2. For each segment index *i* in order: `start = parseFilenameLocalTimestamp(segment.timestamp)` (skip if null);
   `offset = d.ordinalSeconds − start.ordinalSeconds` (integer seconds; milliseconds are dropped).
   In range if `0 ≤ offset < 60` for a non-final segment, `0 ≤ offset ≤ 60` for the final segment.
   The first in-range segment wins → return `i·60 + offset`.
3. No segment matches (before the first, after the last, or in a gap between segments) → null.

`sentryInitialElapsedSeconds(event)` = 0 unless `category == 'sentry'` and detection is non-null, then
`max(0, detection − 15)`.

`sentryPlaybackStartLabel(event)` = null unless Sentry; `e = sentryInitialElapsedSeconds`, `k = ⌊e/60⌋`; null if
segment *k* does not exist; else `formatFilenameTimestampAtOffset(segments[k].timestamp, e − 60k)`.

Examples (`event-detection.test.ts`): `[12:00:00]` + `12:00:05` → 5; `[12:00:00, 12:01:00]` + `12:01:20` → 80,
pre-roll 65, label `2026-08-24 12:01:05`; `[12:00:00, 12:01:11]` + `12:01:05` → null (gap);
`[12:00:00, 12:00:50]` + `12:00:55` → 55 (earliest wins); `[12:00:00, 12:01:00]` + `12:01:00` → 60;
same + `12:02:00` → 120 (final endpoint); `[23:59:30, 00:00:30(+1d)]` + `00:00:45Z` → 75;
`invalid`, `…+09:00`, `12:01:01` with one segment → null. A Saved event with a valid detection keeps
`sentryInitialElapsedSeconds = 0` and `sentryPlaybackStartLabel = null`.

Where detection is used:

| Use | Condition |
|---|---|
| Detection marker on the position slider (§2.10.8) | `detectionSeconds ≠ null` (any category) and not representative mode |
| Inspector `検知時刻` row (raw `metadata.timestamp` string) | `detectionSeconds ≠ null` (any category) |
| Inspector `再生開始` row | `sentryPlaybackStartLabel ≠ null` (Sentry only) |
| Event row `検知の15秒前から開始` | row selected, Sentry, `detectionSeconds ≠ null` |
| Initial playback position | `sentryInitialElapsedSeconds` |

### 1.17 Playback time math (`playback/timeline.ts`)

Every segment occupies a nominal 60 s window regardless of actual clip lengths or real gaps between segments.

`locatePlaybackTime(event, t)`:

1. `D = eventDuration(event)`; if `D == 0` → `{segmentIndex: 0, offsetSeconds: 0}`.
2. `t' = NaN → 0`, then clamp to `[0, D]` (−∞ → 0, +∞ → D).
3. `segmentIndex = min(⌊t'/60⌋, segments.length − 1)`; `offsetSeconds = t' − 60·segmentIndex`.

So boundaries belong to the next segment (60 → `{1, 0}`), except the end: with 3 segments 180 → `{2, 60}`.
Normalized elapsed is always `60·segmentIndex + offsetSeconds`.

### 1.18 Synchronized playback state machine (`playback/*`)

The Mac MAY use any media stack (AVFoundation players, host-time synchronized rates, decoded frame output into GPUI),
but the **observable behavior** below MUST match. A pure Rust model of this machine SHOULD exist so it can be tested
with the contract fixtures in §6 without real media.

#### 1.18.1 State

Per theater session:

- `elapsedSeconds` (normalized), `segmentIndex`, `rate` (initial 1), `playing` (UI flag), `message` (null or one of
  the two messages below).
- `intended` — play intent. **Initialized to true** when the session is created (autoplay, §1.19).
- Per registered camera: `lifecycle ∈ {loading, ready, terminal}`, membership in `readyCameras`, the latest play
  attempt (`pending | fulfilled | rejected`, tagged with intent/source/stream generations), membership in
  `successfulCameras`.
- `sourceCamera` — the camera whose media clock drives `elapsedSeconds`.

Messages (`use-synchronized-playback.ts`; `\n` is a line break that MUST be rendered):

- `NO_PLAYABLE` = `再生できるカメラ映像がありません。\n読み込み後に再試行してください。`
- `REJECTED` = `映像を再生できませんでした。\nブラウザの再生許可を確認して再試行できます。`

#### 1.18.2 Resolution (`resolvePlayback`)

```text
if !intended                                   → paused
else if successfulCameras non-empty            → playing
else if any registered camera has a pending attempt
     or any registered camera is loading
     or no camera is registered                → waiting
else                                           → failed
```

After every refresh: `playing = (resolution == playing)`; `playing` → `message = null`; `failed` →
`intended = false`, attempts and successes cleared, `message = REJECTED`. `waiting`/`paused` leave `message` unchanged.

#### 1.18.3 Commands

| Command | Effect |
|---|---|
| `play()` | `intended = true`; clear attempts/successes; `message = null`; P = ready cameras in canonical order; if P is empty `message = NO_PLAYABLE`; start a play attempt on every camera in P; refresh; when attempts settle, record fulfilled/rejected only for attempts that are still current (same camera element, intent, source and stream generation); refresh. Net effect: `NO_PLAYABLE` survives only while some camera is still loading (`waiting`); if all cameras are terminal the refresh resolves `failed` and `REJECTED` replaces it (§8 A-3) |
| `pause()` | `intended = false`; clear attempts/successes; `playing = false`; pause every player. `message` unchanged |
| `seek(t)` | `loc = locatePlaybackTime(t)`. If `loc.segmentIndex ≠ segmentIndex`: pause every player, bump the source generation, clear attempts/successes, `playing = false` (intent is kept). Then `elapsedSeconds = 60·loc.segmentIndex + loc.offsetSeconds`, set every registered player's media time to `loc.offsetSeconds` and its rate to `rate` |
| `skip(Δ)` | `seek(elapsedSeconds + Δ)`; Δ is ±10 in the UI |
| `setRate(r)` | `rate = r`; apply to every registered player. Allowed values: 0.5, 1, 1.5, 2 |

#### 1.18.4 Media events

Events come from the player of camera *c*. Events from a player that is no longer registered for *c* are ignored.
Before handling any event, re-elect the source (§1.18.5).

| Event (web origin) | Effect |
|---|---|
| `loading` (new source assigned / src cleared) | invalidate *c*'s attempt and success (bump its stream generation); `lifecycle = loading`; remove from ready; re-elect if *c* was the source; refresh |
| `ready` (`canplay`) | `firstReady = lifecycle ≠ ready`; `lifecycle = ready`; add to ready; re-elect; if `firstReady`, set *c*'s media time to the offset of `locatePlaybackTime(elapsedSeconds)`; apply `rate`; if `intended` and (`firstReady` or (*c* has no attempt and is not successful)) start a play attempt on *c* only. A repeated `ready` never rewinds and never replays |
| `unavailable` (missing file, file read failure, media error) | invalidate *c*; `lifecycle = terminal`; remove from ready; re-elect if needed; refresh |
| `timeupdate` | only if *c* is the source: `abs = 60·segmentIndex(elapsed) + c.mediaTime`; `loc = locate(abs)`; `elapsedSeconds = 60·loc.segmentIndex + loc.offsetSeconds`; for every other **ready** camera with `|mediaTime − loc.offsetSeconds| > 0.25` set its media time to `loc.offsetSeconds` (exactly 0.25 is not corrected). If the source's media time passes 60 s in a non-final segment, the position moves into the next segment (the stage then loads that segment) |
| `ended` | only if *c* is the source: `next = 60·(segmentIndex + 1)`; if `next < duration` → `seek(next)`; else `seek(duration)` then `pause()` |

Consequences: a 20 s source clip in a 60 s window jumps to the next window when it ends; at the final end the slider
shows the full nominal duration and playback is paused.

Pressing play when the source's media time is at its end restarts the media of the **current** segment from offset 0
(web media elements restart on `play()` after `ended`; `interactions.spec.ts` relies on it). The Mac MUST seek ended
players to 0 before starting them (§8 A-36).

#### 1.18.5 Source election (`playback-source-election.ts`)

`preferredCamera` = the **expanded** camera (null when no camera is expanded; the merely *active* camera is not
preferred). If the preferred camera is ready it becomes the source; otherwise the first ready camera in canonical
order; otherwise none.

#### 1.18.6 Session end and media resources

On session end (selected event changes, rescan, app state leaves `ready`): stop (`intended = false`), pause all
players, drop registrations and release every media handle. Media handles are reference-counted per file
(`ObjectUrlRegistry`): acquiring the same file twice reuses one handle; the last release frees it; disposal frees all.
Switching view mode, expanding/collapsing a camera, or changing the active camera MUST NOT recreate players, reload
media, or release handles (`Theater.test.tsx`: 6 acquisitions, 0 releases across mode switches). A segment change
releases the previous segment's files.

### 1.19 Autoplay and selection rules

- Selecting an event whose id differs from the current one creates a new theater session with
  `elapsedSeconds = locate(sentryInitialElapsedSeconds(event))` normalized, `intended = true`, `rate = 1`, no expanded
  camera, active camera `front`, all camera states reset, export job reset. Each camera autoplays once on its first
  `ready`, seeked to the initial offset (Sentry: 15 s before detection; Saved/Recent: 0).
- The rule is identical for every entry point: event row, day-timeline marker or cluster member, previous/next
  buttons, day navigation, and keyboard.
- Re-selecting the already selected event does nothing (no rewind, no replay, position kept).
- An explicit `pause()` before streams are ready cancels autoplay; later `ready` events do not start playback.
- View mode (`spatial`/`grid`) is workspace state and is **kept** across event changes; it resets to `spatial` when a
  new folder is loaded.
- If every play attempt is rejected or every registered camera becomes terminal while playing is intended, playback
  stops with `REJECTED`; later `ready` events do not restart it until the user presses play.

### 1.20 Day timeline (`day-timeline.ts`, `DayTimeline*.tsx`, `use-timeline-presentation.ts`)

Inputs: `timelineEvents` (§1.15), the selected event, the filter `dateKey`, the selected event id.

#### 1.20.1 Date and markers

- `eventTimestamp(event)` = first segment timestamp for Recent, else `event.timestamp` (identical in practice).
- `selectTimelineDate(events, selected, filterDateKey)`: (1) the selected event's `dateKey` if it parses; else
  (2) `filterDateKey` if `"{dateKey}_00-00-00"` is a valid filename timestamp; else (3) the first event in `events`
  whose timestamp parses; else null. Inside a theater the selected event always exists, so (1) applies.
- `buildDayTimelineMarkers(events, dateKey)`: null/invalid `dateKey` → `[]`. Otherwise one marker per event whose
  parsed date equals `dateKey`: `{eventId, category, dateKey, timeLabel, secondsOfDay,
  positionPercent = secondsOfDay / 86 399 × 100}` (00:00:00 → 0, 23:59:59 → 100), sorted by `secondsOfDay`, then
  category order `sentry < saved < recent`, then `eventId` ascending (byte-wise is equivalent for generated ids).

#### 1.20.2 Recorded days and adjacency

- `buildTimelineDays(events)`: group markers by `dateKey`, days ascending, each day's `eventIds` in marker order;
  unparsable timestamps are skipped.
- `adjacentTimelineDays(days, current)`: previous = greatest day `< current`, next = smallest day `> current`
  (string comparison of `YYYY-MM-DD`). Invalid `current` → both undefined.
- Selecting a day (`前の記録日` / `次の記録日` buttons) dispatches `date-filtered {dateKey: day.dateKey}` followed by
  `event-selected {eventId: day.eventIds.last}` — the **latest** marker of that day (ties: the last in marker order) —
  and therefore autoplays it.

#### 1.20.3 Clusters (rail presentation)

`buildDayTimelineClusters(events, dateKey, trackWidthPixels)` with **`trackWidthPixels = 640` fixed**
(`minimumDesktopTrackWidth`; the real rail width is not measured, §8 A-6):

- Walk markers in order; a marker joins the current cluster when
  `((m.positionPercent − previous.positionPercent) / 100) × trackWidthPixels < 44` (chained against the previous
  marker, not the cluster start). With 640 this means a gap ≤ 5 939 s joins and ≥ 5 940 s separates.
  Non-finite or ≤ 0 widths never cluster.
- Cluster = `{id: first marker eventId, count, markers, startTimeLabel, endTimeLabel, startSecondsOfDay,
  endSecondsOfDay, durationSeconds = end − start, startPositionPercent, endPositionPercent}`.
- Display position of a cluster = midpoint of start and end percent.

#### 1.20.4 Presentation

`rail` when the viewport is wider than 1024 px, `ledger` for 640–1024 px, `mobile` below 640 px. The Mac desktop
window is always `rail` (§2.12). Cluster expansion (`expandedClusterId`) and the mobile history toggle are local to the
timeline and reset whenever the theater session is recreated (selecting another event collapses the open cluster;
`App.selection.test.tsx` then focuses the cluster trigger that contains the new selection).

### 1.21 App state machine (`app-state.ts`)

States: `idle {filters}`, `scanning {progress}`, `error {reason ∈ denied|invalid|failed, filters}`,
`ready {folderName, library, filters, selectedEventId}`. Initial: `idle` with initial filters.

| Action | From | To |
|---|---|---|
| `scan-started` | any | `scanning {processed: 0, total: 0}` (the previous library is discarded) |
| `scan-progressed {progress}` | `scanning` | `scanning {progress}`; ignored elsewhere |
| `scan-cancelled` | any | initial `idle` (even from `ready`, §8 A-1) |
| `scan-denied` / `scan-invalid` / `scan-failed` | any | `error {denied / invalid / failed}` with initial filters |
| `library-ready {folderName, library}` | any | `error {invalid}` if the library has 0 events, else `ready` with initial filters and `selectedEventId = null` |
| `category-filtered` / `date-filtered` / `query-filtered` | `ready` | updated filters; selection cleared if the selected event no longer matches; ignored elsewhere |
| `event-selected {eventId}` | `ready` | `selectedEventId = eventId` (not validated); ignored elsewhere |

The web dispatches `scan-started` before the native picker opens, so the scanning view is visible while the picker is
open. The Mac MAY enter `scanning` only after the open panel returns, but the end states MUST match the table
(cancelling a re-pick from `ready` returns to the welcome screen).

---

## 2. Screens and states

Layout numbers come from `styles/*.css` and `tokens.css`; token names refer to §4. "Desktop" below means a viewport or
window content width of at least 1025 pt.

### 2.1 Window and global rules

- Window title: `TeslaCam Viewer` (web `<title>`).
- **Light appearance only.** The web forces `color-scheme: light`, a light `theme-color` and PWA colors, and has no
  theme switch (`theme.spec.ts`). The Mac MUST render with the light (Aqua) appearance even when macOS is in Dark Mode.
- Proposed window sizes (Mac decision, §8 A-10): default content size 1280 × 1000 pt (the e2e desktop viewport);
  minimum content size 1025 × 760 pt so the stacked tablet and mobile layouts never apply.
- Root layout is one of four views chosen by app state (§1.21): welcome (`idle`), welcome-with-error (`error`),
  scanning (`scanning`), workspace (`ready`).
- Focus ring: 3 pt solid `--color-focus` outside the control with 2 pt offset on keyboard focus.
- Minimum interactive size 44 × 44 pt for every visible control (`interactions.spec.ts` asserts it for all visible
  buttons).
- Disabled buttons: 38 % opacity, not clickable (day-date navigation uses 55 %, §2.10.9).
- Reduced motion (macOS "Reduce motion"): spinners stop rotating; nothing else animates in either app. No layout,
  position or size animation is allowed anywhere (DESIGN.md §6). Hover/active color changes MAY be instantaneous
  (the documented `--motion-fast` 160 ms token is not implemented on the web, §8 A-13).
- Iconography: the web uses Lucide icons (`FolderOpen`, `LoaderCircle`, `CalendarDays`, `Search`, `ShieldAlert`,
  `ChevronDown`, `Video`, `ImageOff`, `ChevronLeft`, `ChevronRight`, `Play`, `Pause`, `RotateCcw`, `RotateCw`,
  `History`, `Film`, `X`, `Download`). The Mac SHOULD use the same Lucide glyphs (SVG assets) at the same sizes;
  SF Symbols equivalents are acceptable if visually equivalent. Icons are always paired with text.
- **[N/A on Mac]** Compatibility status banner (`ブラウザ互換性` region with
  `このブラウザでは互換モードを使用します。フォルダ選択画面からTeslaCamフォルダを選んでください。`) — shown on the web only when
  `showDirectoryPicker` is missing and the app is not ready. The Mac always has a native picker.

### 2.2 Welcome (state `idle`)

Structure (`EmptyState.tsx`, `welcome.css`), top to bottom, left-aligned in a column of at most 640 pt:

| Element | Content | Style |
|---|---|---|
| Brand | `TeslaCam Viewer` | 16 pt, weight 600, letter-spacing −0.015 em; 16 pt bottom padding; 1 pt bottom border `--color-border` |
| Title (h1) | `SSDのTeslaCamフォルダを開く`; the tail `フォルダを開く` never wraps internally | `--font-display`, weight 600, line-height 1.08, letter-spacing −0.03 em, max width 15 ch |
| Description | `セントリー、保存済み、最近の走行を日時順に整理して、最大6方向の映像を確認します。` | 18 pt (`--font-body-large`), `--color-text-secondary`, line-height 1.7 |
| Primary action | `FolderOpen` 20 pt icon + `TeslaCamフォルダを選ぶ` | min 236 × 46 pt, padding 0 20, radius 6, fill `--color-accent`, 1 pt border `--color-accent-contrast`, text `--color-text-on-accent` weight 600, icon gap 8; hover `--color-accent-hover`, pressed `--color-accent-active` |
| Outcome list | `選ぶもの` → `RecentClips、SavedClips、SentryClipsを含むTeslaCamフォルダ`; `表示されるもの` → `日時順のイベント一覧と、最大6方向の同期映像` | top border; each row a 112 pt label column + flexible value, 16 pt gap and vertical padding, bottom border; label 14 pt `--color-text-muted`, value 14 pt `--color-text-secondary` line-height 1.6 |
| Browser guidance | `MacのChromeまたはEdgeに対応しています。` | 14 pt `--color-text-muted`. **Mac: omitted by default** (it describes browsers; §8 A-14) |

Spacing: 32 pt between blocks; page padding 64 pt vertical and `max(24, (width − 1180) / 2)` horizontal; the column
is vertically centered. Activating the primary action starts folder selection (§1.3). There is exactly one primary
action on this screen.

### 2.3 Scanning (state `scanning`)

`ScanProgress.tsx`: a centered panel (max 560 pt) with a rotating `LoaderCircle` (28 pt, `--color-accent-border`,
1.2 s per turn, linear), heading `フォルダを確認しています` (24 pt), and a details line (`--color-text-secondary`):

| Progress | Details text |
|---|---|
| `processed == 0` | `フォルダを読み込んでいます。` |
| `processed == total` | `{processed} ファイルを確認中です。` |
| otherwise | `{processed} / {total} ファイルを確認中です。` |

Numbers here (only here) use `ja-JP` digit grouping: `1,234`. During the walk `total == processed` (the total is
unknown), so the short form appears; during classification the `processed / total` form appears. The panel is a
polite live region; the Mac SHOULD post an accessibility announcement when the text changes.

### 2.4 Errors (state `error`)

Same layout as §2.2, without the outcome list (the guidance line follows the §2.2 rule). Title, description and button
label by reason; the button restarts folder selection.

| Reason | Trigger | Title | Description | Button |
|---|---|---|---|---|
| `denied` | permission refused (§1.3) | `フォルダへのアクセスが許可されませんでした。` | `TeslaCamフォルダへの読み取りアクセスを許可して、もう一度選んでください。` | `もう一度選ぶ` |
| `invalid` | 0 files, or 0 events (wrong folder, e.g. only `notes.txt`) | `TeslaCamフォルダを確認できませんでした。` | `RecentClips、SavedClips、SentryClipsを含むフォルダを選んでください。` | `もう一度選ぶ` |
| `failed` | any read error, unknown picker failure, every scan-limit error | `フォルダの読み取りを完了できませんでした。` | `SSDの接続を確認してから、もう一度選んでください。` | `もう一度選ぶ` |

Cancelled selection shows the plain welcome screen (heading `SSDのTeslaCamフォルダを開く`), not an error.

### 2.5 Workspace layout (state `ready`)

```text
┌──────────────────────────────── AppHeader (56) ────────────────────────────────┐
│ TeslaCam Viewer                                          [⌂ 別のフォルダを選ぶ] │
├──────────── EventNavigator (320) ─┬───────────────── Theater (flex) ───────────┤
│ LibrarySidebar (auto, scrolls)    │ TheaterContext (min 64)                    │
│  読み込み元 / folder / N 件のイベント│  time · category | facts | switch · export │
│  映像: すべて / 最近の走行 / …      ├──────── CameraStage (flex) ──┬ Inspector ─┤
│  日付 [date]   検索 [query]        │  spatial 3×3 or grid 3×2      │   (240)    │
│  ▸ 読み込みの注意  n件              │  (dark evidence surface)      │  scrolls   │
├───────────────────────────────────┤                               │            │
│ EventList (flex, scrolls)         ├───────────────────────────────┴────────────┤
│  イベント                    N件   │ [RepresentativeVideo — only in fallback]   │
│  rows …                           ├──────────── PlaybackDeck (224 / 268) ──────┤
│                                   │ PlaybackControls (min 108)                 │
│                                   │ DayTimeline (rest, dark)                   │
└───────────────────────────────────┴────────────────────────────────────────────┘
```

- Workspace height = window content height − 56; it never scrolls as a whole. The sidebar, event list and inspector
  scroll internally.
- With no selected event, the theater area shows the selection placeholder (§2.9) instead of the theater.
- At 1280 × 1000 every region (navigator, camera stage, inspector, playback deck) and every visible timeline control
  MUST lie inside the window without overlap (`rich-layout.spec.ts`).

### 2.6 App header

`AppHeader.tsx`: one row, min height 56 pt, `--color-rail` background, bottom border, horizontal padding 20 pt.
Left: `TeslaCam Viewer` (weight 600). Right: quiet button `FolderOpen` 18 pt + `別のフォルダを選ぶ` (min height 44,
padding 0 16, 1 pt border `--color-border-strong`, radius 6, transparent, `--color-text-secondary`; hover fill
`--color-surface-active`, text `--color-text-primary`). The button restarts folder selection (§1.21: the current
library is discarded immediately). The folder name is **not** repeated in the header; it appears once, in the sidebar
(`layout.spec.ts`: the exact text `TeslaCam` occurs once). The Mac MAY merge this row into a unified title bar if both
elements stay visible and the height stays ≤ 64 pt.

### 2.7 Library sidebar

`LibrarySidebar.tsx`, `--color-rail`, padding 20 × 16, 24 pt gaps, scrolls internally.

1. **Source** (accessibility name `読み込み元`): label `読み込み元` (12 pt, weight 560, `--color-text-muted`);
   folder name (14 pt, weight 600, single line, ellipsis); `{eventCount} 件のイベント` (12 pt muted, half-width space
   before `件`; `eventCount` = all events). 16 pt bottom padding and bottom border.
2. **Category navigation** (accessibility name `映像カテゴリ`), label `映像`, four rows:
   `すべて`, `最近の走行`, `保存済み`, `セントリー`, each with its count on the right (12 pt muted, tabular digits).
   Counts come from `eventsByCategory` and are **not** affected by filters (`すべて` = sum). Row: min height 44,
   padding 0 12, 3 pt left border (transparent; `--color-accent-border` when active), active fill
   `--color-surface-active`, text `--color-text-secondary` (primary when active or hovered; hover fill
   `--color-surface`). Activating a row dispatches `category-filtered`. Accessibility: the active row is marked current.
3. **Filters**:
   - `日付` with `CalendarDays` 16 pt icon: a date field. Committing a valid date dispatches `date-filtered` with
     `YYYY-MM-DD`; clearing it dispatches `date-filtered null`. The web uses `<input type=date>`; the Mac control MUST
     support clearing and MUST only commit valid calendar dates. Display format is free (`YYYY-MM-DD` is recommended).
   - `検索` with `Search` 16 pt icon: text field, placeholder `日時またはカテゴリ`; every edit dispatches
     `query-filtered` with the raw text (no debounce).
   - Fields: full width, min height 44, padding 0 12, 1 pt `--color-border-strong`, radius 6, `--color-surface`.
4. **Scan warnings** (only if there is at least one warning): a disclosure pinned to the bottom of the sidebar,
   **collapsed by default**. Summary row (44 pt): `ShieldAlert` 18 pt + `読み込みの注意` (12 pt, weight 560,
   `--color-warning`) and, on the right, `{warnings.length}件` (number of warning **kinds**, `--color-text-muted`).
   Expanded body: one 12 pt `--color-text-secondary` line per warning (§1.10). 1 pt top border `--color-warning`.

### 2.8 Event list

`EventList.tsx`, `--color-surface`, vertical borders.

- Header (56 pt, bottom border): title `イベント` (16 pt, 600); right: `{matchingEvents.length}件` (14 pt muted).
  The disclosure button (`イベント一覧を閉じる` / `イベント一覧を開く`) exists only at ≤ 1024 px **[N/A on Mac]**.
- Empty result: `この条件に一致するイベントはありません。` (14 pt muted, padding 24 × 16).
- Rows (`matchingEvents` order, newest first; accessibility group `イベント一覧`). Each row is one button whose
  accessibility label is `eventTime` (e.g. `2026-08-20 20:45:00`) and whose pressed state reflects selection:
  - Thumbnail (only if `thumbnailFile` loads and decodes): 88 × 60 pt, aspect-fill (`object-fit: cover`), left of the
    text. No placeholder frame or error icon when there is no thumbnail or it fails. Its image handle is released
    when the row goes away.
  - Line 1: `eventDate` (body size, unstyled).
  - Line 2: `eventClock` (14 pt, weight 600, tabular digits, single line, ellipsized if needed; full value in the
    tooltip) and the category badge (`Video` 14 pt icon + label, 12 pt, weight 560, `--color-text-secondary`,
    `--color-sentry` for Sentry; never wraps or shrinks; the time column shrinks first).
  - Line 3 facts (12 pt muted, wrapping, separated by `·` in `--color-border-strong` with 8 pt spacing):
    duration label, segment label, availability label.
  - Missing line (12 pt `--color-warning`) when `missingCameraLabel` is non-null.
  - Pre-roll line `検知の15秒前から開始` (12 pt `--color-sentry`) when the row is selected, Sentry, and detection is valid.
  - Row: min height 96, padding 12 × 16, gap 12, 3 pt left border (`--color-accent-border` when selected), bottom
    divider, hover `--color-surface-raised`, selected `--color-surface-active`. Rows are list entries, not cards.
- Activating a row dispatches `event-selected` (§1.19). (Web also collapses the list at ≤ 1024 px **[N/A on Mac]**.)

### 2.9 Selection placeholder

When no event is selected: centered heading `イベントを選択してください` (24 pt) and
`一覧から選ぶと、記録された映像をここに表示します。` (`--color-text-muted`, line-height 1.6), padding 32, on the
`--color-theater` background. The web uses light-theme text colors on this dark surface (contrast ≈ 1.2:1); the Mac
MUST use `--color-theater-text-primary` / `--color-theater-text-secondary` instead (§8 A-31).

### 2.10 Theater (selected event)

The theater (accessibility name `6カメラシアター`) is a vertical stack on `--color-surface`.

#### 2.10.1 Context bar (`TheaterContext.tsx`, accessibility name `再生中のイベント`)

One row, min height 64 pt, padding 8 × 16, bottom border; three columns `[min 220 pt, flexible] [auto] [auto]`:

1. `eventTime` heading (16 pt, 600, tabular) + category badge.
2. Facts (12 pt muted, right-aligned, wraps, max 28 rem, each fact single-line): duration, segments, availability;
   the missing label (if any) on its own full-width line in `--color-warning`, right-aligned.
3. Actions (right-aligned, 4 pt gap): view switch (§2.10.2) then the export control (§2.11). The export control's
   bounds MUST lie inside the context bar.

#### 2.10.2 View switch (`TheaterViewSwitch.tsx`, group `表示レイアウト`)

Two toggle buttons `車両配置` (spatial, default) and `6分割` (grid); pressed state = current mode. Each min 88 × 44,
padding 0 12, transparent, 2 pt bottom border (`--color-accent-border` when pressed), text `--color-text-secondary`
(primary when pressed). Both are disabled while a camera is expanded. The switch is not shown in representative mode
(§2.10.7) and not shown when the window is narrower than 1280 pt (§2.12). The mode is kept across events.

#### 2.10.3 Camera stage (dark evidence surface)

Inside the stage the theme tokens are remapped (§4.2): rail `#101318`, text `#f5f6f7`/`#c2c8d0`, borders
`#303741`/`#74808e`, accent border `#8bb9ff`, warning `#e1a74d`. Tiles are separated by 1 pt gaps showing
`--color-theater-border`.

- **Spatial** (`camera-stage[data-view-mode=spatial]`, ≥ 1280 pt only): 3 equal columns × 3 equal rows (each ≥ 120 pt).
  Areas: `· front ·` / `left_pillar vehicle right_pillar` / `left_repeater back right_repeater`. The vehicle cell shows
  `vehicle-top.png` (360 × 720 RGBA, transparent background) at 40 % of the cell width, at most 80 % of the cell
  height, aspect-fit, centered. The empty top-left and top-right cells show the gap color.
- **Grid**: 3 equal columns × 2 equal rows (each ≥ 120 pt) in canonical order; no vehicle image. Below 1280 pt the
  stage always renders as grid even if the stored mode is spatial.
- **Expanded**: one camera fills the whole stage; the other five tiles stay alive but hidden (players, positions,
  readiness and media handles are kept); the vehicle image is hidden. The web's CSS for "expanded while spatial" is
  ill-defined (§8 A-11); the Mac MUST render expanded identically in both modes.
- The same six tile instances (and players) are used in every arrangement; changing arrangement never reloads media
  (`rich-layout.spec.ts` checks node identity).
- Video frames are always aspect-fit (`object-fit: contain`), never cropped; letterbox area is `--color-theater`.

#### 2.10.4 Camera tiles (`CameraTile.tsx`)

Each tile is a button with accessibility label `{label}カメラを拡大` (or `{label}カメラをグリッドへ戻す` when expanded)
and pressed = expanded. Activating it sets the active camera to this camera and toggles expansion
(expanded → collapse; otherwise expand this one).

- Frame: 1 pt border `#74808e`; `#8bb9ff` when active or expanded; square corners; background `--color-theater`.
- Camera name chip: top-left, inset 8 pt, padding 4 × 8, background `#101318`, text `#c2c8d0` 12 pt.
- Status overlay, shown whenever status ≠ `ready`: covers the tile from 48 pt below the top to the bottom, centered,
  padding 32/12/12, background `--color-theater`, 14 pt text; icon `LoaderCircle` 18 pt rotating for `loading`,
  `ImageOff` 20 pt otherwise; text color `#c2c8d0`, or `#e1a74d` for `unavailable`, `corrupt`, `unsupported`.
  The label chip and the overlay text MUST NOT overlap (`error-states.spec.ts`).

| Status | Label | Entered when |
|---|---|---|
| `loading` | `読み込み中` | the current segment has a file for this camera and it is being opened (initial state when a file exists) |
| `ready` | `再生できます` (never displayed: no overlay in this state) | first frame can be presented (`canplay`) |
| `missing` | `映像がありません` | the current segment has no file for this camera |
| `unavailable` | `映像を読み込めません` | the file cannot be read (e.g. removed SSD), or a media error that is neither decode nor format |
| `corrupt` | `映像が破損しています` | media decode error (`MEDIA_ERR_DECODE`) |
| `unsupported` | `未対応の映像形式です` | unsupported format/source (`MEDIA_ERR_SRC_NOT_SUPPORTED`) |

Web error mapping: decode → `corrupt`; src-not-supported → `unsupported`; network or no code → `unavailable`;
aborted → ignored while a source is being replaced, else `unavailable`; any other code → `unavailable`. Mac mapping
(guidance): AVFoundation "decode failed" → `corrupt`; "file format not recognized", "failed to parse", "decoder not
found" → `unsupported`; file-system/permission errors and anything else → `unavailable`. A 4-byte garbage `.mp4` MUST
end as `corrupt` or `unsupported` (the e2e accepts either). Every terminal status reports `unavailable` to the
playback machine (§1.18.4).

When the segment changes, each tile switches to the new segment's file for its camera (or `missing`), releasing the
previous one.

#### 2.10.5 Active camera and camera switcher

`activeCamera` starts at `front` per session; it is set by clicking a tile or by digit shortcuts and only affects the
active border. The mobile camera switcher (group `表示するカメラ`, buttons `{label}を表示`) exists in the DOM at every
width but is visible only below 640 px **[N/A on Mac]**.

#### 2.10.6 Event inspector (`EventInspector.tsx`, accessibility name `イベント詳細`)

Desktop: a permanent 240 pt column right of the stage, `--color-surface`, left border, padding 16, scrolls; always
expanded and without its summary row (the web's collapsed `<details>` summary — `イベント詳細`, `場所を確認`,
`{category} · {clock} · {availability}` — exists only at ≤ 1024 px **[N/A on Mac]**).

Rows in this order (label 12 pt muted; value 14 pt, wraps anywhere; 12 pt between rows, 4 pt label→value):

| Label | Value | Shown when |
|---|---|---|
| `場所（Teslaの推定値）` | `city.trim()` and `street.trim()`, non-empty parts joined by one space | either is non-empty |
| `推定座標` | `"{est_lat.trim()}, {est_lon.trim()}"` (monospace, tabular) | both non-empty |
| `検知時刻` | the raw `metadata.timestamp` string (e.g. `2026-08-20T20:35:30`) | `detectionSeconds ≠ null` |
| `再生開始` | `{sentryPlaybackStartLabel}（検知の15秒前）` | Sentry with valid detection |
| `連結再生時間` | duration label | always |
| `セグメント` | segment label | always |
| `カメラ` | availability label | always |
| `欠損カメラ` | missing label | something is missing |
| `カテゴリ` | category label | always |
| `録画開始` | `eventTime` | always |
| `記録情報` | a nested disclosure `技術情報` (collapsed by default) revealing `reason` in monospace 12 pt muted | `metadata.reason` is present (even if empty) |

The inspector only reads the selected event (it does not change with the active camera or segment) and never
contacts any service (no maps, no geocoding, no links).

#### 2.10.7 Representative fallback (`RepresentativeVideo.tsx`)

Representative mode = all six tile statuses are terminal (`missing`, `unavailable`, `corrupt`, `unsupported`; a tile
that has not reported yet counts as `loading`) **and** the event has `event.mp4`. In this mode:

- The view switch is hidden, the vehicle image is hidden, the detection marker is hidden.
- The stage becomes one strip of six tiles (one row, 124 pt tall, at least 104 pt); tile name chips ellipsize.
- A section (accessibility name `代表映像`) appears between the stage row and the deck: 12 × 16 padding, 1 pt top
  border `--color-warning`, background `--color-warning-surface`, two columns `[min 180 pt, 32 %] [rest]`, 16 pt gap.
  Left: `代表映像（単独再生）` (strong) and `6カメラ同期映像ではありません。` (12 pt secondary; `ありません。` never wraps).
  Right: a single muted video with standard transport controls, aspect-fit, max height 220 pt, theater background.
  If the file cannot be read: status text `代表映像を読み込めません。` and no player.
- The representative player is independent of the synchronized machine. Vertical order MUST be
  tiles → representative → transport, all inside the window (`error-states.spec.ts`).

#### 2.10.8 Playback controls (`PlaybackControls.tsx`, accessibility name `再生コントロール`)

Inside the deck, `--color-rail`, min height 108 pt, padding 4 × 16, gaps 4 × 16, top border. Grid areas
`timeline rate` / `primary primary` / `message message`.

- **Timeline row**: elapsed label (42 pt column), position slider, duration label (42 pt column); 12 pt muted, tabular.
  Labels use `M:SS` with unpadded minutes: `⌊s/60⌋:pad2(⌊s mod 60⌋)` (65 → `1:05`, 300 → `5:00`).
  Slider (`再生位置`): range 0…duration, step 0.1, value = elapsed, accent `--color-accent-border`; dragging calls
  `seek(value)` continuously.
- **Detection marker** (`検知位置`): non-interactive 3 × 18 pt bar in `--color-warning`, centered vertically on the
  slider track at `detectionSeconds / duration × 100 %` of the track (1 segment + detection 30 → 50 %;
  2 segments + detection 80 → 66.67 %); absent when detection is null, duration is 0, or in representative mode.
- **Rate**: label `速度` + popup (`再生速度`, min width 76) with `0.5×`, `1×`, `1.5×`, `2×` (U+00D7); default `1×`;
  resets to `1×` with each new session.
- **Primary row**: five equal buttons (min 44 × 44, 4 pt gap, 1 pt `--color-border-strong`, radius 6,
  `--color-surface`, 12 pt `--color-text-secondary`, icon + text 4 pt gap; hover when enabled: `--color-surface-active`
  and primary text):

| Visible text | Accessibility label | Icon | Action | Enabled |
|---|---|---|---|---|
| `過去` | `過去の記録` | `ChevronLeft` (before text) | select the next-older event in `matchingEvents` (index + 1) | selected index ≥ 0 and not last |
| `-10秒` | `10秒戻る` | `RotateCcw` | `skip(−10)` | always |
| `再生` / `一時停止` | `再生` / `一時停止` | `Play` / `Pause` | `play()` / `pause()` | always |
| `+10秒` | `10秒進む` | `RotateCw` | `skip(+10)` | always |
| `新しい` | `新しい記録` | `ChevronRight` (after text) | select the next-newer event (index − 1) | selected index > 0 |

  The play button is filled `--color-accent` with `--color-text-on-accent`, weight 600, `--color-accent-contrast` border.
- **Message** (`playback.message`): 12 pt `--color-warning`, line breaks preserved, polite status.

#### 2.10.9 Day timeline, rail presentation (`DayTimeline.tsx`, `DayTimelineRail.tsx`)

Fills the rest of the deck (deck height 224 pt, 268 pt while a cluster is expanded). Dark surface `#101318`, text
`#f5f6f7`, secondary `#c2c8d0`, borders `#303741`; accessibility name `日内イベントタイムライン`.

- **Header** (min 44): `日内イベント` (12 pt) and the shown date `dateKey` (12 pt secondary; `日付を選択してください` if
  null). Right: navigation group `記録日を移動` with `ChevronLeft` + `前の記録日` and `次の記録日` + `ChevronRight`
  (accessibility labels `前の記録日 {dateKey}` / `次の記録日 {dateKey}` when the day exists, plain `前の記録日` /
  `次の記録日` and disabled otherwise). Buttons: 44 tall, padding 0 8, 1 pt `#74808e`, radius 6, 12 pt; disabled:
  border `#303741`, secondary text, 55 % opacity. Action per §1.20.2.
- **Empty day**: `この日のイベントはありません。` (14 pt secondary) replaces everything below the header.
- **Expanded cluster row** (only while a cluster is open, above the ruler): group labelled
  `{HH:MM}–{HH:MM}・{count}件` (en dash U+2013, katakana middle dot U+30FB), 4 pt padding and gap, 1 pt `#74808e`
  border, radius 8, horizontal scrolling; one event button per marker (min 112 × 44, padding 0 12, border, radius 6).
- **Ruler**: `00:00` left, `12:00` center, `24:00` right; 12 pt secondary; 16 pt side padding; decorative.
- **Markers area** (group `この日の記録`, min 44, 16 pt side margins, 1 pt top border): for each cluster, at its
  position (percent of the area width, horizontally centered on that point):
  - single-marker cluster → an event button;
  - multi-marker cluster → a cluster trigger: text `{HH:MM}–{HH:MM}` and `{count}件`, accessibility label
    `{HH:MM}–{HH:MM}・{count}件を開く` / `…を閉じる`, expanded state; min 72 × 44, 1 pt `#74808e`, 2 pt left border
    `--color-accent-border` (`#155eef`), radius 6, 12 pt tabular, no wrapping; fill `#050608` when it contains the
    selected event. Activating it toggles that cluster (only one cluster is open at a time).
- **Event button** (markers and cluster members, `TimelineEventButton.tsx`): text `{timeLabel}` (`HH:MM:SS`) and the
  category label; accessibility label `{dateKey} {timeLabel} {categoryLabel}`; pressed = selected.
  Marker style: min 44 × 44, padding 0 4, 2 pt left border `#155eef` (`#c62828` for Sentry), fill `#101318`
  (`#050608` when selected), 12 pt tabular. Cluster-member style: border `#74808e` (`#8bb9ff` and fill `#050608` when
  selected). Activating dispatches `event-selected`, then keyboard focus returns to that event's button, or to the
  trigger of the collapsed cluster that now contains it.
- **Keyboard** (focus on an event button): `←`/`→` move focus to the previous/next marker of the day,
  `Home`/`End` to the first/last; the target is the event's own button if visible, else its cluster trigger.
  Focus moves only; selection does not change.
- **Neighbour summary** (`イベントの前後関係`: `過去: …`, `選択: …`, `新しい: …`, `同日: {n}件`) is rendered by the web
  only at ≤ 1024 px **[N/A on Mac]**; see §5 for its strings.

#### 2.10.10 Keyboard shortcuts (`use-theater-shortcuts.ts`)

Active while a theater exists; handled at window level.

1. `Escape` while a camera is expanded → collapse it (always, even when a control has focus). Otherwise `Escape` is
   not consumed.
2. If a control holds keyboard focus — text field, date field, popup/select, any button (including camera tiles,
   transport buttons and timeline markers), or editable text — stop here (the control keeps the key). On the web,
   clicking a button focuses it; the Mac MUST behave the same for tiles, transport buttons and timeline markers, so a
   click followed by `→` does not skip (`Theater.shortcuts.test.tsx`).
3. `Space` → `pause()` if playing, else `play()`.
4. `←` → `skip(−10)`; `→` → `skip(+10)`.
5. `1`…`6` (main row or keypad) → active camera := `cameraIds[n−1]` and toggle its expansion.
6. Handled keys do not propagate further.

Mac decisions (§8 A-21): key combinations with Command, Control or Option are not handled as shortcuts (they belong to
menus); while the export dialog is open the theater shortcuts are suspended.

### 2.11 Composite export UI (`CompositeVideoExport.tsx`, `CompositeExportDialog.tsx`)

The export control lives in the context-bar actions. It is owned by the theater session: changing the selected event
aborts a running export silently and resets the control.

States and transitions (`exportReducer`):

```text
idle{dialogOpen:false, format:null, notice:null}
  ── trigger ──▶ resolve format ──null──▶ error{'このブラウザは動画書き出しに対応していません。'}
                               └─ok──▶ idle{dialogOpen:true, format, notice:null}       (dialog shown)
idle{dialogOpen:true} ── キャンセル / close / Escape ──▶ idle{dialogOpen:false}           (focus back to trigger)
idle{dialogOpen:true} ── 書き出しを開始 ──▶ exporting{progress:0}                       (dialog closes)
exporting ── progress p ──▶ exporting{clamp(p,0,1)}
exporting ── 中止 ──▶ idle{dialogOpen:false, format:null, notice:'書き出しを中止しました。'}
exporting ── done ──▶ complete{result}          exporting ── error ──▶ error{message}
complete ── 閉じる ──▶ idle (initial)            error ── もう一度 ──▶ resolve format again (as the trigger)
```

| State | UI |
|---|---|
| idle | Trigger button: `Download` 16 pt + `書き出す`, accessibility label `合成動画を書き出す`; 44 tall, padding 0 12, 1 pt `--color-border-strong`, radius 6, transparent, 12 pt `--color-text-secondary`. Disabled when export is unavailable, and then followed by the reason (12 pt `--color-warning`): `6カメラ映像がないため書き出せません` when `cameraAvailability.available == 0`, else `このブラウザは書き出しに未対応です` (unreachable on Mac). If a notice exists it is shown after the trigger (12 pt, polite status) |
| dialog | Modal, see below |
| exporting | `書き出し中` + `{MM:SS} / {MM:SS}` (polite live region), a progress bar (accessibility `書き出しの進行状況`, 0…1, width `min(240 pt, 22 % of window width)`, accent color) and a `中止` button |
| complete | Status text `書き出し完了 · {EXT} · 1280 × 720` (U+00B7 separators), a filled accent button `Download` + `{EXT}を保存`, and `閉じる`. `{EXT}` = upper-cased extension (`MP4`; the web may also produce `WEBM`) |
| error | Alert text (`--color-warning`) with the error message and a `もう一度` button |

Clock format here: `pad2(⌊s/60⌋):pad2(⌊s mod 60⌋)` (minutes zero-padded to at least 2 digits), with
`s = progress × eventDuration` and total `eventDuration` (nominal 60 s per segment, §8 A-8). Example: 1 segment,
progress 0.5 → `00:30 / 01:00`.

Export availability: `cameraAvailability(event).available > 0` (some camera exists in every segment) and the platform
can encode. The Mac can always encode, so only the first condition applies (§8 A-7).

Error messages shown in the error state are the export error's message (§3.7), or
`合成動画を書き出せませんでした。もう一度お試しください。` when there is none. Aborts are never shown as errors.

**Dialog** (modal, accessibility title `合成動画を書き出す`, description = the first paragraph):

- Width `min(520 pt, window − 32 pt)`, 1 pt `--color-border-strong`, radius 8, `--color-surface`; backdrop
  `--color-scrim` (rgba 5, 6, 8, 0.64).
- Header (padding 12 × 16, bottom border): `Film` 18 pt + title `合成動画を書き出す` (16 pt); close button `X`
  (44 × 44, accessibility label `書き出し画面を閉じる`).
- Body (padding 20 × 16, 16 pt gap, 14 pt `--color-text-secondary`, line-height 1.6):
  1. `選択中の記録を、6カメラと車両配置を含む1本の動画にまとめます。利用できないカメラは「記録なし」と表示します。`
     (the two sentences and the `1本の動画にまとめます。` fragment do not wrap internally).
  2. Details list (top/bottom borders, padding 12 × 0, rows `[88 pt label] [value]`, label muted, value primary
     tabular):
     `記録` → `{eventTime} · {categoryLabel}`; `範囲` → `イベント全体・{durationLabel}`; `解像度` → `1280 × 720`;
     `形式` → `{format.label}` plus `（失敗時は{fallbackLabel}）` when a fallback exists (web only). Mac: `MP4`.
  3. Notice (`--color-warning`): `対応ブラウザでは高速に書き出します。完了まではタブを閉じないでください。`
     (two non-wrapping sentences; browser-worded, §8 A-14).
- Footer (right-aligned, top border, 44 pt buttons, padding 0 16, 14 pt, radius 6): `キャンセル` (transparent,
  secondary) and `書き出しを開始` (accent fill).
- `Escape` = `キャンセル`. After the dialog closes by any cancel path, keyboard focus returns to the trigger
  (`composite-export.spec.ts`).

**Save**: `{EXT}を保存` writes the finished file; the Mac shows a save panel with the default name
`TeslaCam-{event.timestamp}-vehicle.mp4` (e.g. `TeslaCam-2026-08-20_20-45-00-vehicle.mp4`; web:
`TeslaCam-{timestamp}-vehicle.{ext}` download). The result stays available for repeated saves until `閉じる` or the
session ends. An aborted export never yields a saveable file.

### 2.12 Responsive rules

In scope for the Mac (window content width; heights are free above the minimum):

| Band | Behavior |
|---|---|
| ≥ 1280 pt | Full desktop: spatial mode available, view switch visible |
| 1025–1279 pt | Desktop columns unchanged (navigator 320, inspector 240, deck 224/268), but the view switch and vehicle image are hidden and the stage is always the 3 × 2 grid; the export control is pushed to the right end of the actions area |

Web-only bands **[N/A on Mac]**, listed for completeness (`responsive.css`, `mobile.css`):

- ≤ 1024 px (tablet): sticky header; whole page scrolls; sidebar becomes a 2-column grid with 4 category tabs and a
  2-column filter row; event list collapsible (max 320 px), auto-collapses after a selection; selecting an event
  scrolls the theater into view; inspector becomes a closed disclosure below the stage whose summary shows
  `イベント詳細` and `{category} · {clock} · {availability}`; stage min height 55 % of the viewport; timeline uses the
  `ledger` presentation (vertical list of event buttons, `↑`/`↓` also move focus) with the neighbour summary row;
  ruler hidden; representative strip 124 px.
- ≤ 639 px (mobile): welcome padding 32 × 20, display title 1.75 rem, full-width primary button; compact header;
  single-column sidebar; one camera visible (the active one) with the horizontal camera switcher `{label}を表示`
  (selecting collapses expansion); stage 46 % of the viewport; transport ≤ 140 px with the `速度` label visually hidden;
  inspector hidden for events without location, otherwise its summary reads `場所を確認` and only location,
  coordinates and `技術情報` are shown; timeline `mobile` presentation with `履歴を開く（{n}件）` /
  `履歴を閉じる（{n}件）`; export dialog becomes a bottom sheet; context bar on two rows.
- ≤ 359 px: category badge icons hidden.
- Forced colors (Windows high contrast) outlines.

---

## 3. Composite export specification

Source: `export/composite-layout.ts`, `composite-visual-tokens.ts`, `composite-webcodecs.ts` (fast path),
`composite-video.ts` / `composite-renderer.ts` / `composite-recorder.ts` (real-time MediaRecorder fallback).
The Mac implements the **fast path semantics** (offline decode → draw → encode); the real-time fallback is
**[N/A on Mac]** except where noted.

### 3.1 Canvas

- 1280 × 720 pixels (fixed; the dialog and completion text state `1280 × 720`).
- Every frame starts by filling the whole canvas with `--color-composite-canvas` `#000000`.

### 3.2 Slot rectangles (pixels; `cameraFrames`, in draw order)

| Order | Camera | x | y | width | height |
|---|---|---:|---:|---:|---:|
| 1 | `front` | 400 | 12 | 480 | 216 |
| 2 | `left_pillar` | 12 | 180 | 384 | 216 |
| 3 | `right_pillar` | 884 | 180 | 384 | 216 |
| 4 | `left_repeater` | 12 | 492 | 384 | 216 |
| 5 | `back` | 400 | 492 | 480 | 216 |
| 6 | `right_repeater` | 884 | 492 | 384 | 216 |
| — | vehicle box | 496 | 260 | 288 | 184 |

Slot labels are the camera labels of §1.1.

### 3.3 Per-slot drawing (`drawCompositeFrame`)

For each slot in draw order:

1. Fill the slot rectangle with `--color-composite-camera-surface` `#000000`.
2. If the camera has a frame for this output frame ("playable"), draw it **contain-fitted** into the slot:
   `scale = min(W/w, H/h)`, `dw = w·scale`, `dh = h·scale`, `dx = x + (W − dw)/2`, `dy = y + (H − dh)/2`
   (no rounding; a source with `w ≤ 0` or `h ≤ 0` uses the slot itself). Examples: 1448 × 938 into `front` →
   (473.279…, 12, 333.441…, 216); 320 × 180 into `front` → (448, 12, 384, 216); 1920 × 1080 into (0, 0, 300, 300) →
   (0, 65.625, 300, 168.75). Letterbox areas stay black (`composite-export.spec.ts` samples pixel (410, 100): every
   RGB channel ≤ 8).
3. Label text = `{label}` if playable, else `{label}・記録なし` (U+30FB), e.g. `リア・記録なし`.
4. Label geometry (tokens §4.4): font `500 13px` system UI font (`-apple-system, BlinkMacSystemFont, "Hiragino Sans",
   "Yu Gothic UI", "Yu Gothic", sans-serif`; Mac: system font, medium weight, 13 pt, Hiragino Sans fallback for
   Japanese); `labelX = x + 8`, `labelY = y + 8`; `labelWidth = ceil(textAdvanceWidth) + 2·8`; scrim rectangle
   `(labelX, labelY, labelWidth, 24)` filled with `rgba(0, 0, 0, 0.72)`; text drawn in `#ffffff` with its alphabetic
   baseline origin at `(labelX + 8, labelY + 17)`, left-aligned. Example: `back` slot → text origin (416, 517).

After all slots, draw the vehicle image (`vehicle-top.png`, 360 × 720, alpha preserved) contain-fitted into the
vehicle box → `(594, 260, 92, 184)`. The vehicle is drawn last. The image is loaded once per export; if it cannot be
loaded the export fails with `車両イメージを読み込めませんでした。`.

The canvas tokens are resolved once per export and not re-read while rendering.

### 3.4 Timing and concatenation (fast path)

- Output frame rate 30 fps; frame duration 1/30 s.
- Process segments in order. For segment *s*, prepare every camera that has a file in *s*: open it, require a
  decodable primary video track and a finite positive duration (metadata duration, else computed); cameras that fail
  are silently left out (they render as `記録なし`).
- A segment with no prepared cameras is **skipped**: it contributes no frames and no time.
- `segmentDuration = max(camera durations)` (actual media durations, not the nominal 60 s);
  `frameCount = ceil(segmentDuration · 30)`.
- For output frame *i* (0 ≤ *i* < frameCount): each camera with `i < ceil(cameraDuration · 30)` supplies its video
  frame at media time `i / 30` (the latest decoded frame whose presentation time is ≤ `i/30`, i.e. sample-and-hold);
  a camera past its own frame count, or without a frame at that time, is not playable for that frame.
- Output timestamp of frame *i* = `segmentStart + i/30`; its duration = `min(1/30, segmentDuration − i/30)`;
  `segmentStart` accumulates the durations of the segments already written. The output is therefore the
  concatenation of the real segment durations (e2e: five 2 s segments → output longer than 9.5 s; the playback UI
  shows the same event as `5分`).
- If no frame was produced for the whole event, the export fails with `再生可能な動画がありません。`.

### 3.5 Encoding and container

- Video only (no audio track). H.264/AVC, 1280 × 720, constant 30 fps, keyframe interval 2 s (every 60 frames),
  quality-oriented encoding (web: mediabunny `Quality('high')`, `latencyMode: 'quality'`). A specific bitrate is not
  part of the contract; 4–8 Mbit/s average is recommended.
- Container MP4 (ISO BMFF), MIME `video/mp4`, extension `mp4`, format label `MP4`.
- The resulting file MUST decode in AVFoundation and Chromium at 1280 × 720 with duration > 0.
- Performance: an export of real-time duration *T* SHOULD finish faster than 0.9 × *T* (e2e: 10 s of media under 9 s).
- Web-only: when the fast path is unavailable or fails (except aborts), the web falls back to real-time MediaRecorder
  recording (MP4 if `MediaRecorder.isTypeSupported`, else WebM: `video/mp4;codecs=avc1.42E01E,mp4a.40.2`, `video/mp4`,
  `video/webm;codecs=vp9,opus`, `video/webm;codecs=vp8,opus`, `video/webm`), drawing at most 30 fps, pausing the
  recorder between segments and while the page is hidden. **[N/A on Mac]**

### 3.6 Progress semantics

- Report 0 when encoding starts.
- During segment *s* (0-based, out of `segmentCount = event.segments.length`, skipped segments included), after
  writing frame *i* with `i % 30 == 0` or `i == frameCount − 1`, report `(s + (i + 1)/frameCount) / segmentCount`.
- Report 1 after the file is finalized. The UI clamps to [0, 1] and only reacts while exporting.
- Progress therefore may jump over skipped segments and never decreases.

### 3.7 Errors and cancellation

| Code | Message | When |
|---|---|---|
| `unsupported` | `このブラウザは動画書き出しに対応していません。` | no encoder (unreachable on Mac) |
| `unsupported` | `このブラウザはCanvas録画に対応していません。` | web canvas/capture missing **[N/A on Mac]** |
| `recording-failed` | `車両イメージを読み込めませんでした。` | vehicle asset failed to load |
| `recording-failed` | `動画の録画に失敗しました。` | encoder/writer failure |
| `recording-failed` | `動画を再生できないため、書き出しを開始できません。` | web real-time path: no video could start **[N/A on Mac]** |
| `no-playable-media` | `再生可能な動画がありません。` | the event produced no frames |
| `no-playable-media` | `動画を読み込めませんでした。` | web real-time path, per clip (swallowed) **[N/A on Mac]** |
| `aborted` | `動画の書き出しをキャンセルしました。` | user pressed `中止` or the session ended; never displayed |

Cancellation stops decoding and encoding promptly, deletes any partial output, and releases every file handle and
decoder. Cancelling before any media is touched MUST NOT open any clip.

### 3.8 Output naming

Default file name `TeslaCam-{event.timestamp}-vehicle.mp4`, where `{event.timestamp}` is the raw filename timestamp
of the event's first segment.

---

## 4. Design tokens

Source of truth: `styles/tokens.css` (values) and `DESIGN.md` (roles). The proposed machine-readable copy is
`packages/contract/tokens/design-tokens.json` (§6.4); the Mac theme MUST be generated from or tested against it.
Light theme only. No shadows, blur, glow, gradients or glass anywhere (DESIGN.md §7).

### 4.1 Colors (light workspace)

| Token | Value | Role |
|---|---|---|
| `--color-canvas` | `#f7f9fc` | Window/page background (also web `theme-color` and PWA colors) |
| `--color-rail` | `#eef2f6` | Header, library rail, playback controls/deck |
| `--color-surface` | `#ffffff` | Event list, inputs, context bar, inspector, dialog |
| `--color-surface-raised` | `#f3f6fa` | Hover on event rows |
| `--color-surface-active` | `#e7eef8` | Selected/active rows, hover on quiet buttons |
| `--color-theater` | `#050608` | Video surfaces, letterboxing, selected timeline marker |
| `--color-text-primary` | `#17202a` | Headings, body |
| `--color-text-secondary` | `#465466` | Secondary text, default button text |
| `--color-text-muted` | `#5d6b7a` | Metadata, labels, counts |
| `--color-text-on-accent` | `#ffffff` | Text on accent fills |
| `--color-border` | `#aeb9c6` | Surface boundaries, dividers |
| `--color-border-strong` | `#657384` | Control boundaries |
| `--color-accent` | `#155eef` | Primary action, play button, save button, start-export button |
| `--color-accent-hover` | `#0f4cc9` | Primary hover |
| `--color-accent-active` | `#0b3b9e` | Primary pressed |
| `--color-accent-border` | `#155eef` | Selected boundaries, slider accent |
| `--color-accent-contrast` | `#d8ebff` | 1 pt outline of accent-filled buttons |
| `--color-sentry` | `#c62828` | Sentry category text, Sentry timeline marker, pre-roll line (always with text) |
| `--color-warning` | `#8a4b00` | Missing cameras, scan warnings, detection marker, playback message, export errors |
| `--color-warning-surface` | `#fff4e5` | Representative fallback background |
| `--color-focus` | `#005fcc` | Keyboard focus ring |
| `--color-theater-rail` | `#101318` | Camera name chips, day timeline background |
| `--color-theater-text-primary` | `#f5f6f7` | Primary text on dark surfaces |
| `--color-theater-text-secondary` | `#c2c8d0` | Secondary text on dark surfaces |
| `--color-theater-border` | `#303741` | Tile gaps, timeline dividers |
| `--color-theater-border-strong` | `#74808e` | Tile and timeline control boundaries |
| `--color-theater-accent-border` | `#8bb9ff` | Active/expanded tile, selected cluster member |
| `--color-theater-warning` | `#e1a74d` | Terminal tile status text |
| `--color-scrim` | `rgb(5 6 8 / 64%)` = rgba(5, 6, 8, 0.64) | Export dialog backdrop |
| `--color-composite-canvas` | `#000000` | Export canvas background |
| `--color-composite-camera-surface` | `#000000` | Export slot background / letterbox |
| `--color-composite-label-scrim` | `rgb(0 0 0 / 72%)` = rgba(0, 0, 0, 0.72) | Export label background |
| `--color-composite-label-text` | `#ffffff` | Export label text |

### 4.2 Scoped token remaps

| Scope | Token → value |
|---|---|
| Camera stage (`.camera-grid`) | `--color-rail` → `#101318`; `--color-text-primary` → `#f5f6f7`; `--color-text-secondary` → `#c2c8d0`; `--color-border` → `#303741`; `--color-border-strong` → `#74808e`; `--color-accent-border` → `#8bb9ff`; `--color-warning` → `#e1a74d` |
| Day timeline (`.day-timeline`) | `--color-text-primary` → `#f5f6f7`; `--color-text-secondary` → `#c2c8d0`; `--color-border` → `#303741` (accent border stays `#155eef`) |

### 4.3 Typography

Root size 16 pt (1 rem). Font: the system UI font (SF Pro) with Hiragino Sans for Japanese; no bundled web fonts
(§8 A-12). Monospace `--font-mono`: `ui-monospace, "SFMono-Regular", Menlo, Consolas, monospace` (Mac: SF Mono).

| Level | Token | Size | Weight | Line height | Tracking | Used for |
|---|---|---:|---:|---:|---:|---|
| Display | `--font-display` | `clamp(32, 4 % of window width, 48)` pt | 600 (DESIGN: 560, §8 A-27) | 1.08 | −0.03 em | Welcome/error title |
| Display mobile | `--font-display-mobile` | 28 pt | 560 | 1.1 | −0.03 em | [N/A on Mac] |
| Heading | `--font-heading` | 24 pt | 600 | 1.25 | −0.02 em | Scanning title, placeholder title |
| Body large | `--font-body-large` | 18 pt | 400 | 1.7 | −0.01 em | Welcome description |
| Body | `--font-body` | 16 pt | 400 | 1.6 | 0 | Default text, list header, context time, dialog title |
| Small | `--font-small` | 14 pt | 400–450 | 1.5–1.6 | 0 | Folder name, inspector values, event time, dialog body, guidance |
| Label | `--font-label` | 12 pt | 400 / 560 | 1.35 | 0.01 em | Category badges, facts, counts, transport/timeline text, chips |
| Composite label | `--font-composite-label` | 13 px | 500 | — | 0 | Export video labels only |

Weights in use: 400 (body), 560 (labels, category badges, warnings summary), 600 (`strong`, headings, primary action,
play button). Numeric displays (times, counts, durations) use tabular figures.

### 4.4 Composite canvas tokens (`composite-visual-tokens.ts` defaults = `tokens.css`)

| Token | Value |
|---|---|
| `--color-composite-canvas` | `#000000` |
| `--color-composite-camera-surface` | `#000000` |
| `--color-composite-label-scrim` | `rgba(0, 0, 0, 0.72)` |
| `--color-composite-label-text` | `#ffffff` |
| `--font-composite-label` | `500 13px -apple-system, BlinkMacSystemFont, "Hiragino Sans", "Yu Gothic UI", "Yu Gothic", sans-serif` |
| `--size-composite-label-inset` | 8 px |
| `--size-composite-label-padding-inline` | 8 px |
| `--size-composite-label-height` | 24 px |
| `--size-composite-label-baseline` | 17 px |

### 4.5 Spacing (4 pt base)

| Token | Value | Token | Value |
|---|---:|---|---:|
| `--space-1` | 4 | `--space-6` | 24 |
| `--space-2` | 8 | `--space-8` | 32 |
| `--space-3` | 12 | `--space-10` | 40 |
| `--space-4` | 16 | `--space-12` | 48 |
| `--space-5` | 20 | `--space-16` | 64 |
| | | `--space-20` | 80 |

### 4.6 Radii and sizes

| Token | Value | Usage |
|---|---:|---|
| `--radius-control` | 6 | Buttons, inputs, selects, timeline controls |
| `--radius-panel` | 8 | Dialog, expanded cluster row |
| `--control-min-size` | 44 | Minimum control height/width |
| `--app-header-height` | 56 | Header |
| `--event-navigator-width` | 320 | Left column |
| `--event-inspector-width` | 240 | Inspector column |
| `--camera-stage-spatial-min` | 720 | Stage width that allows spatial mode (reached at 1280 pt windows) |
| `--playback-deck-height` | 224 | Deck (controls + timeline) |
| `--playback-deck-expanded-height` | 268 | Deck while a cluster is open |
| `--event-thumbnail-width` | 88 | Row thumbnail width (height 60) |
| `--event-card-min-height` | 96 | Event row |
| `--theater-tile-min` | 120 | Stage row minimum |
| `--theater-control-height` | 44 | Transport buttons |
| `--theater-switcher-height` | 44 | [N/A on Mac] camera switcher |
| `--theater-event-list-max` | 320 | [N/A on Mac] tablet list |
| `--theater-representative-max` | 40 % of viewport height | [N/A on Mac] mobile representative |
| `--theater-mobile-expanded-min` | 60 % of viewport height | [N/A on Mac] |
| `--theater-video-fit` | `contain` | Every camera video |
| `--transport-mobile-max` | 140 | [N/A on Mac] |
| `--composite-export-progress-inline-size` | `min(240, 22 % of window width)` | Export progress bar |
| `--composite-export-dialog-inline-size` | 520 | Dialog width |
| `--composite-export-detail-label-inline-size` | 88 | Dialog label column |

Other fixed dimensions used in §2: primary action min 236 × 46; welcome column 640; scan panel 560; outcome label
column 112; context bar min 64; view-switch button min width 88; event-list header 56; playback controls min 108;
time label columns 42; rate select min 76; detection marker 3 × 18; timeline marker min 44 × 44; cluster trigger min
72; cluster member min 112; representative strip 124 (min 104); representative player max height 220; tile status
overlay top offset 48. Documented in DESIGN.md but not defined in CSS: `--theater-terminal-tile-min` 80,
`--theater-tablet-min` 720, `--theater-mobile-tile-min` 180, `--theater-mobile-tile-target` 30 % viewport height,
`--motion-fast` 160 ms `cubic-bezier(0.2, 0, 0, 1)`.

### 4.7 Contrast requirements (from e2e)

- Welcome background luminance > 0.8 and body text contrast ≥ 4.5:1.
- Light surfaces (sidebar, event list, context bar, controls) luminance > 0.75; stage and tiles luminance < 0.05.
- Boundary vs fill of every interactive control (active tile, filter inputs, active category, selected row, quiet
  button, enabled transport buttons) ≥ 3:1.

---

## 5. User-visible Japanese strings

Every string below MUST be rendered byte-for-byte as written (full-width punctuation, `·` U+00B7, `・` U+30FB,
`–` U+2013, `×` U+00D7, `、` U+3001, `（）` U+FF08/U+FF09, half-width spaces where shown). `{x}` placeholders are
filled as noted. Column **Mac**: ✓ render verbatim; **a11y** accessibility name (SHOULD be exposed as an
accessibility label; MAY double as a tooltip); **N/A** web-only, not rendered on the Mac; **omit** Mac decision to not
render (§8 A-14); **browser** browser-worded copy rendered verbatim by default (§8 A-14).

The proposed IDs are the keys for `packages/contract/strings/ja.json`.

### 5.1 Global, welcome, scanning, errors

| ID | String | Where | Mac |
|---|---|---|---|
| `app.name` | `TeslaCam Viewer` | Window title, welcome brand, header | ✓ |
| `compat.region` | `ブラウザ互換性` | Compatibility banner name | N/A |
| `compat.message` | `このブラウザでは互換モードを使用します。フォルダ選択画面からTeslaCamフォルダを選んでください。` | Compatibility banner | N/A |
| `compat.input` | `TeslaCamフォルダを選ぶ（互換モード）` | Hidden fallback input label | N/A |
| `welcome.title` | `SSDのTeslaCamフォルダを開く` | Welcome h1 (tail `フォルダを開く` kept on one line) | ✓ |
| `welcome.description` | `セントリー、保存済み、最近の走行を日時順に整理して、最大6方向の映像を確認します。` | Welcome | ✓ |
| `welcome.action` | `TeslaCamフォルダを選ぶ` | Welcome primary button | ✓ |
| `welcome.selectTerm` | `選ぶもの` | Outcome list label | ✓ |
| `welcome.selectValue` | `RecentClips、SavedClips、SentryClipsを含むTeslaCamフォルダ` | Outcome list value | ✓ |
| `welcome.resultTerm` | `表示されるもの` | Outcome list label | ✓ |
| `welcome.resultValue` | `日時順のイベント一覧と、最大6方向の同期映像` | Outcome list value | ✓ |
| `welcome.browserGuidance` | `MacのChromeまたはEdgeに対応しています。` | Welcome and error screens | omit |
| `error.retry` | `もう一度選ぶ` | Error button | ✓ |
| `error.denied.title` | `フォルダへのアクセスが許可されませんでした。` | Error h1 | ✓ |
| `error.denied.description` | `TeslaCamフォルダへの読み取りアクセスを許可して、もう一度選んでください。` | Error | ✓ |
| `error.invalid.title` | `TeslaCamフォルダを確認できませんでした。` | Error h1 | ✓ |
| `error.invalid.description` | `RecentClips、SavedClips、SentryClipsを含むフォルダを選んでください。` | Error | ✓ |
| `error.failed.title` | `フォルダの読み取りを完了できませんでした。` | Error h1 | ✓ |
| `error.failed.description` | `SSDの接続を確認してから、もう一度選んでください。` | Error | ✓ |
| `scan.title` | `フォルダを確認しています` | Scanning h1 | ✓ |
| `scan.starting` | `フォルダを読み込んでいます。` | Scanning, processed = 0 | ✓ |
| `scan.count` | `{processed} ファイルを確認中です。` | Scanning, processed = total (ja-JP grouping) | ✓ |
| `scan.countOfTotal` | `{processed} / {total} ファイルを確認中です。` | Scanning (ja-JP grouping) | ✓ |

### 5.2 Workspace chrome, sidebar, list

| ID | String | Where | Mac |
|---|---|---|---|
| `header.rescan` | `別のフォルダを選ぶ` | Header button | ✓ |
| `navigator.region` | `イベントナビゲーション` | Navigator name | a11y |
| `source.label` | `読み込み元` | Sidebar label and region name | ✓ |
| `source.eventCount` | `{count} 件のイベント` | Sidebar | ✓ |
| `categories.region` | `映像カテゴリ` | Category nav name | a11y |
| `categories.label` | `映像` | Category nav label | ✓ |
| `category.all` | `すべて` | Category row | ✓ |
| `category.recent` | `最近の走行` | Category row, badges, timeline, search haystack | ✓ |
| `category.saved` | `保存済み` | same | ✓ |
| `category.sentry` | `セントリー` | same | ✓ |
| `filter.date` | `日付` | Date filter label | ✓ |
| `filter.search` | `検索` | Search label | ✓ |
| `filter.searchPlaceholder` | `日時またはカテゴリ` | Search placeholder | ✓ |
| `warnings.summary` | `読み込みの注意` | Warnings disclosure | ✓ |
| `warnings.kindCount` | `{count}件` | Warnings disclosure (number of kinds) | ✓ |
| `warning.emptyMedia` | `空の映像ファイル {count} 件を除外しました。` | Warnings body | ✓ |
| `warning.invalidMetadata` | `読み取れないイベント情報 {count} 件を除外しました。` | Warnings body | ✓ |
| `warning.unknownFile` | `対応外のファイル {count} 件を除外しました。` | Warnings body | ✓ |
| `warning.encrypted` | `暗号化されたファイル {count} 件は読み込みませんでした。` | Warnings body | ✓ |
| `list.title` | `イベント` | List header (also the list region name) | ✓ |
| `list.count` | `{count}件` | List header | ✓ |
| `list.collapse` | `イベント一覧を閉じる` | Disclosure button | N/A |
| `list.expand` | `イベント一覧を開く` | Disclosure button | N/A |
| `list.region` | `イベント一覧` | Row group name | a11y |
| `list.empty` | `この条件に一致するイベントはありません。` | Empty list | ✓ |
| `list.preRoll` | `検知の15秒前から開始` | Selected Sentry row | ✓ |
| `placeholder.title` | `イベントを選択してください` | No selection | ✓ |
| `placeholder.description` | `一覧から選ぶと、記録された映像をここに表示します。` | No selection | ✓ |

### 5.3 Formatted values (`event-summary.ts`, `event-filters.ts`)

| ID | Template | Example |
|---|---|---|
| `camera.front` … `camera.rightRepeater` | `フロント`, `リア`, `左ピラー`, `左リピーター`, `右ピラー`, `右リピーター` | — |
| `summary.availability` | `{available}/{total} カメラ` | `6/6 カメラ` |
| `summary.missing` | `{labels joined by 、}なし` | `右リピーターなし` |
| `summary.segments` | `{n}セグメント` | `11セグメント` |
| `duration.seconds` | `{s}秒` | `0秒` |
| `duration.minutes` | `{m}分` | `11分` |
| `duration.hours` | `{h}時間` | `1時間` |
| `duration.hoursMinutes` | `{h}時間{m}分` | `1時間1分` |
| `event.time` | `{YYYY-MM-DD} {hh:mm:ss}` | `2026-08-20 20:45:00` (row accessibility label, context heading, `録画開始`) |

### 5.4 Theater

| ID | String | Where | Mac |
|---|---|---|---|
| `theater.region` | `6カメラシアター` | Theater name | a11y |
| `context.region` | `再生中のイベント` | Context bar name | a11y |
| `viewSwitch.region` | `表示レイアウト` | View switch group | a11y |
| `viewSwitch.spatial` | `車両配置` | View switch | ✓ |
| `viewSwitch.grid` | `6分割` | View switch | ✓ |
| `cameraSwitcher.region` | `表示するカメラ` | Mobile switcher group | N/A |
| `cameraSwitcher.show` | `{label}を表示` | Mobile switcher button | N/A |
| `tile.expand` | `{label}カメラを拡大` | Tile button | a11y |
| `tile.collapse` | `{label}カメラをグリッドへ戻す` | Expanded tile button | a11y |
| `tile.loading` | `読み込み中` | Tile overlay | ✓ |
| `tile.ready` | `再生できます` | Tile status (never displayed) | ✓ |
| `tile.missing` | `映像がありません` | Tile overlay | ✓ |
| `tile.unavailable` | `映像を読み込めません` | Tile overlay | ✓ |
| `tile.corrupt` | `映像が破損しています` | Tile overlay | ✓ |
| `tile.unsupported` | `未対応の映像形式です` | Tile overlay | ✓ |
| `inspector.region` | `イベント詳細` | Inspector name; summary title at ≤ 1024 px | a11y |
| `inspector.locationSummary` | `場所を確認` | Mobile summary | N/A |
| `inspector.summaryContext` | `{category} · {clock} · {availability}` | Summary at ≤ 1024 px | N/A |
| `inspector.location` | `場所（Teslaの推定値）` | Inspector row | ✓ |
| `inspector.coordinates` | `推定座標` | Inspector row | ✓ |
| `inspector.detection` | `検知時刻` | Inspector row | ✓ |
| `inspector.playbackStart` | `再生開始` | Inspector row | ✓ |
| `inspector.playbackStartValue` | `{YYYY-MM-DD hh:mm:ss}（検知の15秒前）` | Inspector value | ✓ |
| `inspector.duration` | `連結再生時間` | Inspector row | ✓ |
| `inspector.segments` | `セグメント` | Inspector row | ✓ |
| `inspector.cameras` | `カメラ` | Inspector row | ✓ |
| `inspector.missing` | `欠損カメラ` | Inspector row | ✓ |
| `inspector.category` | `カテゴリ` | Inspector row | ✓ |
| `inspector.recordingStart` | `録画開始` | Inspector row | ✓ |
| `inspector.record` | `記録情報` | Inspector row | ✓ |
| `inspector.technical` | `技術情報` | Nested disclosure (title and name) | ✓ |
| `representative.region` | `代表映像` | Representative section name | a11y |
| `representative.title` | `代表映像（単独再生）` | Representative section | ✓ |
| `representative.note` | `6カメラ同期映像ではありません。` | Representative section (`ありません。` kept on one line) | ✓ |
| `representative.unavailable` | `代表映像を読み込めません。` | Representative section status | ✓ |

### 5.5 Transport and messages

| ID | String | Where | Mac |
|---|---|---|---|
| `transport.region` | `再生コントロール` | Controls name | a11y |
| `transport.older` | `過去` | Button text | ✓ |
| `transport.olderLabel` | `過去の記録` | Button name | a11y |
| `transport.back10` | `-10秒` | Button text (ASCII hyphen-minus) | ✓ |
| `transport.back10Label` | `10秒戻る` | Button name | a11y |
| `transport.play` | `再生` | Button text and name | ✓ |
| `transport.pause` | `一時停止` | Button text and name | ✓ |
| `transport.forward10` | `+10秒` | Button text | ✓ |
| `transport.forward10Label` | `10秒進む` | Button name | a11y |
| `transport.newer` | `新しい` | Button text | ✓ |
| `transport.newerLabel` | `新しい記録` | Button name | a11y |
| `transport.position` | `再生位置` | Slider name | a11y |
| `transport.detectionMarker` | `検知位置` | Marker name | a11y |
| `transport.rate` | `速度` | Rate label | ✓ |
| `transport.rateLabel` | `再生速度` | Rate popup name | a11y |
| `transport.rateOption` | `{0.5 / 1 / 1.5 / 2}×` | Rate options `0.5×` `1×` `1.5×` `2×` | ✓ |
| `transport.time` | `{m}:{ss}` | Elapsed / duration labels | ✓ |
| `playback.noPlayable` | `再生できるカメラ映像がありません。` + line break + `読み込み後に再試行してください。` | Playback message | ✓ |
| `playback.rejected` | `映像を再生できませんでした。` + line break + `ブラウザの再生許可を確認して再試行できます。` | Playback message | browser |

### 5.6 Day timeline

| ID | String | Where | Mac |
|---|---|---|---|
| `timeline.region` | `日内イベントタイムライン` | Timeline name | a11y |
| `timeline.title` | `日内イベント` | Timeline header | ✓ |
| `timeline.noDate` | `日付を選択してください` | Header when no date | ✓ |
| `timeline.dateNav` | `記録日を移動` | Day navigation group | a11y |
| `timeline.previousDay` | `前の記録日` | Button text; name when disabled | ✓ |
| `timeline.previousDayLabel` | `前の記録日 {dateKey}` | Button name | a11y |
| `timeline.nextDay` | `次の記録日` | Button text; name when disabled | ✓ |
| `timeline.nextDayLabel` | `次の記録日 {dateKey}` | Button name | a11y |
| `timeline.empty` | `この日のイベントはありません。` | Empty day | ✓ |
| `timeline.ruler` | `00:00`, `12:00`, `24:00` | Ruler | ✓ |
| `timeline.markers` | `この日の記録` | Markers group name | a11y |
| `timeline.markerLabel` | `{dateKey} {timeLabel} {categoryLabel}` | Event button name | a11y |
| `timeline.clusterRange` | `{HH:MM}–{HH:MM}` | Cluster trigger text | ✓ |
| `timeline.clusterCount` | `{count}件` | Cluster trigger text | ✓ |
| `timeline.clusterGroup` | `{HH:MM}–{HH:MM}・{count}件` | Expanded cluster row name | a11y |
| `timeline.clusterOpen` | `{HH:MM}–{HH:MM}・{count}件を開く` | Cluster trigger name (collapsed) | a11y |
| `timeline.clusterClose` | `{HH:MM}–{HH:MM}・{count}件を閉じる` | Cluster trigger name (expanded) | a11y |
| `timeline.historyOpen` | `履歴を開く（{count}件）` | Mobile history button | N/A |
| `timeline.historyClose` | `履歴を閉じる（{count}件）` | Mobile history button | N/A |
| `timeline.summaryRegion` | `イベントの前後関係` | Neighbour summary name | N/A |
| `timeline.summaryPrevious` | `過去: {marker}` (name `過去の記録`) | Neighbour summary | N/A |
| `timeline.summarySelected` | `選択: {marker}` (name `選択中の記録`) | Neighbour summary | N/A |
| `timeline.summaryNext` | `新しい: {marker}` (name `新しい記録`) | Neighbour summary | N/A |
| `timeline.summarySameDay` | `同日: {count}件` (name `この日の記録数`) | Neighbour summary | N/A |
| `timeline.summaryMarker` | `{timeLabel} {categoryLabel}` or `なし` | Neighbour summary value | N/A |

### 5.7 Composite export

| ID | String | Where | Mac |
|---|---|---|---|
| `export.trigger` | `書き出す` | Trigger text | ✓ |
| `export.triggerLabel` | `合成動画を書き出す` | Trigger name; dialog title | ✓ |
| `export.dialogClose` | `書き出し画面を閉じる` | Dialog close button name | a11y |
| `export.dialogDescription` | `選択中の記録を、6カメラと車両配置を含む1本の動画にまとめます。利用できないカメラは「記録なし」と表示します。` | Dialog body | ✓ |
| `export.record` | `記録` | Dialog term | ✓ |
| `export.recordValue` | `{eventTime} · {categoryLabel}` | Dialog value | ✓ |
| `export.range` | `範囲` | Dialog term | ✓ |
| `export.rangeValue` | `イベント全体・{durationLabel}` | Dialog value | ✓ |
| `export.resolution` | `解像度` | Dialog term | ✓ |
| `export.resolutionValue` | `1280 × 720` | Dialog value, completion text | ✓ |
| `export.format` | `形式` | Dialog term | ✓ |
| `export.formatValue` | `{label}` / `{label}（失敗時は{fallbackLabel}）` | Dialog value (`MP4` on Mac) | ✓ |
| `export.notice` | `対応ブラウザでは高速に書き出します。完了まではタブを閉じないでください。` | Dialog notice | browser |
| `export.cancel` | `キャンセル` | Dialog footer | ✓ |
| `export.start` | `書き出しを開始` | Dialog footer | ✓ |
| `export.progressTitle` | `書き出し中` | Progress row | ✓ |
| `export.progressClock` | `{MM:SS} / {MM:SS}` | Progress row | ✓ |
| `export.progressLabel` | `書き出しの進行状況` | Progress bar name | a11y |
| `export.abort` | `中止` | Progress row | ✓ |
| `export.aborted` | `書き出しを中止しました。` | Notice after abort | ✓ |
| `export.complete` | `書き出し完了 · {EXT} · 1280 × 720` | Completion text | ✓ |
| `export.save` | `{EXT}を保存` | Save button | ✓ |
| `export.close` | `閉じる` | Completion close | ✓ |
| `export.retry` | `もう一度` | Error retry | ✓ |
| `export.unavailableNoCameras` | `6カメラ映像がないため書き出せません` | Unavailable reason | ✓ |
| `export.unavailableBrowser` | `このブラウザは書き出しに未対応です` | Unavailable reason | N/A |
| `export.missingCameraLabel` | `{label}・記録なし` | Rendered into the video | ✓ |
| `export.error.generic` | `合成動画を書き出せませんでした。もう一度お試しください。` | Export error fallback | ✓ |
| `export.error.unsupported` | `このブラウザは動画書き出しに対応していません。` | Export error | N/A (unreachable) |
| `export.error.canvas` | `このブラウザはCanvas録画に対応していません。` | Export error | N/A |
| `export.error.vehicle` | `車両イメージを読み込めませんでした。` | Export error | ✓ |
| `export.error.recording` | `動画の録画に失敗しました。` | Export error | ✓ |
| `export.error.playback` | `動画を再生できないため、書き出しを開始できません。` | Export error (real-time path) | N/A |
| `export.error.noMedia` | `再生可能な動画がありません。` | Export error | ✓ |
| `export.error.load` | `動画を読み込めませんでした。` | Export error (swallowed) | N/A |
| `export.error.aborted` | `動画の書き出しをキャンセルしました。` | Abort reason (never displayed) | ✓ |

### 5.8 Non-UI metadata strings (reference only)

| Where | String |
|---|---|
| `index.html` meta description | `SSD上のTeslaCam映像を、イベントごとに整理して見やすく再生します。` |
| `manifest.webmanifest` description (no comma) | `SSD上のTeslaCam映像をイベントごとに整理して見やすく再生します。` |
| `manifest.webmanifest` `short_name` | `TeslaCam` |

The Mac MAY show the manifest description in its About panel.

---

## 6. Shared contract for the monorepo (`packages/contract/`)

Goal: every pure rule in §1 and §3 is pinned by golden JSON fixtures that **both** `vitest` (web) and `cargo test`
(Mac) consume, so behavior cannot drift. The contract package holds data only (no code that either app imports at
runtime, except the string catalog and tokens, which the Mac embeds at build time).

### 6.1 Layout

```text
packages/contract/
├── README.md                     ← how to add a fixture, regeneration policy
├── schema/
│   ├── defs.schema.json          ← shared $defs (§6.2)
│   ├── envelope.schema.json      ← fixture file envelope (§6.2)
│   ├── <fixture>.schema.json     ← one per fixture file (§6.3)
│   ├── strings.schema.json
│   └── design-tokens.schema.json
├── fixtures/                     ← golden input → expected output (§6.3)
├── strings/ja.json               ← string catalog (§5, §6.4)
└── tokens/design-tokens.json     ← tokens (§4, §6.4)
```

Consumers:

- Web: `apps/web/src/contract/contract.test.ts` loads `packages/contract/fixtures/*.json`, dispatches each case by
  `fn` to an adapter that calls the real function, and compares with `expected`. Schema validation (Ajv,
  draft 2020-12) runs in the same suite.
- Mac: `apps/mac/crates/teslacam-core/tests/contract.rs` reads the same directory through
  `env!("CARGO_MANIFEST_DIR")`, deserializes with `serde_json`, and dispatches by `fn`.
- Both suites MUST fail when a fixture file, an `fn`, or a case is not handled (no silent skips). A case may
  restrict itself with `"platforms": ["mac"]` or `["web"]` (default: both).
- Root `justfile` tasks (proposal): `contract-validate` (schemas), `contract-test-web`, `contract-test-mac`, and
  `contract-test` running all three.
- Expected values are produced from the web reference by a generator script that lives in the web app
  (`apps/web/scripts/generate-contract-fixtures.ts`, run through a `just` task) and are reviewed by hand; CI only
  verifies (never regenerates). A behavior change requires changing the fixture, the web code, and the Mac code in
  one commit.
- Comparison rules: objects compare by key set and value; arrays by order; numbers with absolute tolerance
  `floatTolerance` (default `1e-9`); the strings `"NaN"`, `"Infinity"`, `"-Infinity"` encode non-finite inputs.

Functions marked **(extract)** below currently live inside React components; the web SHOULD move them into pure
modules (no behavior change) so the fixtures can call them.

### 6.2 Envelope and shared definitions

`schema/envelope.schema.json`:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "envelope.schema.json",
  "type": "object",
  "required": ["contract", "schemaVersion", "module", "source", "cases"],
  "additionalProperties": false,
  "properties": {
    "contract": { "const": "teslacam-viewer" },
    "schemaVersion": { "const": 1 },
    "module": { "type": "string" },
    "source": { "type": "array", "items": { "type": "string" }, "minItems": 1 },
    "floatTolerance": { "type": "number", "exclusiveMinimum": 0, "default": 1e-9 },
    "cases": {
      "type": "array",
      "minItems": 1,
      "items": {
        "type": "object",
        "required": ["name", "fn", "input", "expected"],
        "properties": {
          "name": { "type": "string", "minLength": 1 },
          "fn": { "type": "string" },
          "platforms": { "type": "array", "items": { "enum": ["web", "mac"] }, "minItems": 1, "uniqueItems": true },
          "note": { "type": "string" },
          "input": true,
          "expected": true
        }
      }
    }
  }
}
```

Each fixture schema is `{"allOf": [{"$ref": "envelope.schema.json"}, {"properties": {"cases": {"items": {"$ref": "#/$defs/case"}}}}]}`
plus its own `$defs.case`; §6.3 lists only that `case` definition.

`schema/defs.schema.json`:

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "defs.schema.json",
  "$defs": {
    "CameraId": { "enum": ["front", "back", "left_pillar", "left_repeater", "right_pillar", "right_repeater"] },
    "ClipCategory": { "enum": ["recent", "saved", "sentry"] },
    "CategoryFilter": { "enum": ["all", "recent", "saved", "sentry"] },
    "DateKey": { "type": "string", "pattern": "^[0-9]{4}-[0-9]{2}-[0-9]{2}$" },
    "Num": { "oneOf": [{ "type": "number" }, { "enum": ["NaN", "Infinity", "-Infinity"] }] },
    "GeneratedString": {
      "type": "object", "required": ["$repeat", "count"], "additionalProperties": false,
      "properties": { "$repeat": { "type": "string", "minLength": 1 }, "count": { "type": "integer", "minimum": 0 } },
      "description": "Expands to $repeat concatenated count times; allowed wherever an input string is allowed"
    },
    "Str": { "oneOf": [{ "type": "string" }, { "$ref": "#/$defs/GeneratedString" }] },
    "ScanLimitKind": { "enum": ["entry-count", "directory-depth", "metadata-size", "metadata-string"] },
    "WarningKind": { "enum": ["empty-media-file", "invalid-metadata-file", "unknown-file", "encrypted-file"] },
    "Warning": {
      "type": "object", "required": ["kind", "count"], "additionalProperties": false,
      "properties": { "kind": { "$ref": "#/$defs/WarningKind" }, "count": { "type": "integer", "minimum": 1 } }
    },
    "Progress": {
      "type": "object", "required": ["processed", "total"], "additionalProperties": false,
      "properties": { "processed": { "type": "integer", "minimum": 0 }, "total": { "type": "integer", "minimum": 0 } }
    },
    "Metadata": {
      "type": "object", "required": ["timestamp"], "additionalProperties": false,
      "properties": {
        "timestamp": { "type": "string" }, "city": { "type": "string" }, "street": { "type": "string" },
        "est_lat": { "type": "string" }, "est_lon": { "type": "string" }, "reason": { "type": "string" },
        "camera": { "type": "string" }
      }
    },
    "EventFixture": {
      "type": "object", "required": ["id", "category", "timestamp", "segments"], "additionalProperties": false,
      "properties": {
        "id": { "type": "string" },
        "category": { "$ref": "#/$defs/ClipCategory" },
        "timestamp": { "type": "string" },
        "segments": {
          "type": "array",
          "items": {
            "type": "object", "required": ["timestamp", "cameras"], "additionalProperties": false,
            "properties": {
              "timestamp": { "type": "string" },
              "cameras": { "type": "array", "uniqueItems": true, "items": { "$ref": "#/$defs/CameraId" } }
            }
          }
        },
        "metadata": { "$ref": "#/$defs/Metadata" },
        "hasRepresentative": { "type": "boolean", "default": false },
        "hasThumbnail": { "type": "boolean", "default": false }
      }
    },
    "Rect": {
      "type": "object", "required": ["x", "y", "width", "height"], "additionalProperties": false,
      "properties": { "x": { "type": "number" }, "y": { "type": "number" }, "width": { "type": "number" }, "height": { "type": "number" } }
    },
    "Lifecycle": { "enum": ["loading", "ready", "terminal"] },
    "AttemptStatus": { "enum": ["pending", "fulfilled", "rejected"] },
    "Message": { "enum": ["none", "NO_PLAYABLE", "REJECTED"] }
  }
}
```

`EventFixture` is a compact event: each camera listed in `cameras` gets a placeholder file reference;
`hasRepresentative`/`hasThumbnail` add helper references. Adapters build the platform's real event type from it.

### 6.3 Fixture files

For brevity `D` = `defs.schema.json#/$defs`. Every file uses the envelope; only `$defs.case` is shown.

#### 6.3.1 `clip-filename.json` — `parseClipFileName` (§1.4)

```json
{ "case": { "type": "object", "properties": {
  "fn": { "const": "parseClipFileName" },
  "input": { "type": "object", "required": ["name"], "additionalProperties": false, "properties": { "name": { "type": "string" } } },
  "expected": { "oneOf": [ { "type": "null" }, { "type": "object", "required": ["timestamp", "camera"], "additionalProperties": false,
    "properties": { "timestamp": { "type": "string" }, "camera": { "$ref": "D/CameraId" } } } ] } } } }
```

Cases: six cameras; `event.mp4`, `thumb.png`, `-cabin`, `2026-02-29`, `24-30-45`, `not-a-timestamp-front.mp4`;
upper-case `.MP4`; full-width digits (MUST be null); leap day `2028-02-29`.

#### 6.3.2 `local-timestamp.json` — §1.12

```json
{ "LocalTimestamp": { "type": "object", "additionalProperties": false,
    "required": ["year", "month", "day", "hour", "minute", "second", "millisecond", "dateKey", "timeLabel", "secondsOfDay", "ordinalSeconds"],
    "properties": { "year": { "type": "integer" }, "month": { "type": "integer" }, "day": { "type": "integer" },
      "hour": { "type": "integer" }, "minute": { "type": "integer" }, "second": { "type": "integer" },
      "millisecond": { "type": "integer" }, "dateKey": { "$ref": "D/DateKey" }, "timeLabel": { "type": "string" },
      "secondsOfDay": { "type": "integer" }, "ordinalSeconds": { "type": "integer" } } },
  "case": { "oneOf": [
    { "type": "object", "properties": { "fn": { "enum": ["parseFilenameLocalTimestamp", "parseMetadataLocalTimestamp"] },
      "input": { "type": "object", "required": ["value"], "additionalProperties": false, "properties": { "value": { "type": "string" } } },
      "expected": { "oneOf": [ { "type": "null" }, { "$ref": "#/$defs/LocalTimestamp" } ] } } },
    { "type": "object", "properties": { "fn": { "const": "formatFilenameTimestampAtOffset" },
      "input": { "type": "object", "required": ["value", "offsetSeconds"], "additionalProperties": false,
        "properties": { "value": { "type": "string" }, "offsetSeconds": { "$ref": "D/Num" } } },
      "expected": { "type": ["string", "null"] } } } ] } }
```

Example case: `{"fn": "parseFilenameLocalTimestamp", "input": {"value": "2026-08-24_23-59-59"}, "expected": {"year": 2026,
"month": 8, "day": 24, "hour": 23, "minute": 59, "second": 59, "millisecond": 0, "dateKey": "2026-08-24",
"timeLabel": "23:59:59", "secondsOfDay": 86399, "ordinalSeconds": 63923212799}}`.

#### 6.3.3 `event-metadata.json` — §1.11

```json
{ "MetadataResult": { "oneOf": [
    { "type": "object", "required": ["result", "metadata"], "additionalProperties": false,
      "properties": { "result": { "const": "ok" }, "metadata": { "$ref": "D/Metadata" } } },
    { "type": "object", "required": ["result"], "additionalProperties": false, "properties": { "result": { "const": "invalid" } } },
    { "type": "object", "required": ["result", "kind"], "additionalProperties": false,
      "properties": { "result": { "const": "scan-limit" }, "kind": { "enum": ["metadata-size", "metadata-string"] } } } ] },
  "case": { "oneOf": [
    { "type": "object", "properties": { "fn": { "const": "parseEventMetadata" },
      "input": { "type": "object", "required": ["json"], "additionalProperties": false, "properties": { "json": true } },
      "expected": { "$ref": "#/$defs/MetadataResult" } } },
    { "type": "object", "properties": { "fn": { "const": "readEventMetadata" },
      "input": { "type": "object", "additionalProperties": false,
        "properties": { "text": { "$ref": "D/Str" }, "base64": { "type": "string" }, "declaredSize": { "type": "integer", "minimum": 0 } },
        "description": "Exactly one of text (UTF-8 encoded by the adapter) or base64 (raw bytes); declaredSize defaults to the byte length" },
      "expected": { "$ref": "#/$defs/MetadataResult" } } } ] } }
```

Inside `parseEventMetadata.input.json`, any string value may be a `GeneratedString` (for 2048/2049-unit fields).
Cases: all fields; unknown fields dropped; `null`, `[]`, `42`, `{"timestamp": 42}`, `reason: 42`, `city: null`;
each field at 2048 (ok) and 2049 (`metadata-string`) units, including a surrogate-pair string counted as 2 units;
too-long field plus invalid timestamp (`metadata-string` wins); BOM-prefixed text (ok); invalid JSON (invalid);
`declaredSize` 1 048 577 (`metadata-size`); real bytes 1 048 577 with `declaredSize` 1 (`metadata-size`).

#### 6.3.4 `directory-walk.json` — §1.3 (`pickTeslaCamDirectory` traversal)

```json
{ "Node": { "oneOf": [
    { "type": "object", "required": ["kind", "name", "size"], "additionalProperties": false,
      "properties": { "kind": { "const": "file" }, "name": { "type": "string" }, "size": { "type": "integer", "minimum": 0 } } },
    { "type": "object", "required": ["kind", "name"], "additionalProperties": false,
      "properties": { "kind": { "const": "directory" }, "name": { "type": "string" },
        "entries": { "type": "array", "items": { "$ref": "#/$defs/Node" } },
        "generate": { "type": "object", "required": ["count", "namePattern"], "additionalProperties": false,
          "properties": { "count": { "type": "integer", "minimum": 0 }, "namePattern": { "type": "string", "description": "{i} = 0-based index" },
            "size": { "type": "integer", "minimum": 0, "default": 1 } },
          "description": "Appends count generated files after entries" } } },
    { "type": "object", "required": ["kind", "name"], "additionalProperties": false,
      "properties": { "kind": { "const": "symlink" }, "name": { "type": "string" } } } ] },
  "case": { "type": "object", "properties": {
    "fn": { "const": "walkDirectory" },
    "input": { "type": "object", "required": ["root"], "additionalProperties": false, "properties": { "root": { "$ref": "#/$defs/Node" } } },
    "expected": { "oneOf": [
      { "type": "object", "required": ["status", "folderName", "files", "progress"], "additionalProperties": false,
        "properties": { "status": { "const": "selected" }, "folderName": { "type": "string" },
          "files": { "type": "array", "items": { "type": "object", "required": ["name", "relativePath", "size"], "additionalProperties": false,
            "properties": { "name": { "type": "string" }, "relativePath": { "type": "string" }, "size": { "type": "integer" } } } },
          "progress": { "type": "array", "items": { "$ref": "D/Progress" } } } },
      { "type": "object", "required": ["status", "kind"], "additionalProperties": false,
        "properties": { "status": { "const": "scan-limit" }, "kind": { "enum": ["entry-count", "directory-depth"] } } } ] } } } }
```

`entries` are given in enumeration order (the adapter feeds them in that order; the Mac's name sorting is tested
separately). `symlink` cases carry `"platforms": ["mac"]`. Cases: nested root with hidden file and hidden directory;
root named `RecentClips`/`SavedClips`/`SentryClips`; 0-byte file kept; 251 entries → progress
`[{250,250},{251,251}]`; depth 9 → `directory-depth`; 100 001 entries → `entry-count`; hidden entries counted.

#### 6.3.5 `library-build.json` — §1.5–§1.10 (`buildTeslaCamLibrary`)

```json
{ "InputFile": { "type": "object", "required": ["relativePath"], "additionalProperties": false,
    "properties": { "relativePath": { "type": "string" }, "name": { "type": "string", "description": "default: last '/' component" },
      "size": { "type": "integer", "minimum": 0, "description": "default: byte length of text, else 14" },
      "text": { "$ref": "D/Str", "description": "file contents (used for event.json)" } } },
  "LibraryEvent": { "type": "object", "required": ["id", "category", "timestamp", "segments", "representative", "thumbnail", "metadata"],
    "additionalProperties": false,
    "properties": { "id": { "type": "string" }, "category": { "$ref": "D/ClipCategory" }, "timestamp": { "type": "string" },
      "segments": { "type": "array", "items": { "type": "object", "required": ["timestamp", "cameras"], "additionalProperties": false,
        "properties": { "timestamp": { "type": "string" },
          "cameras": { "type": "object", "propertyNames": { "$ref": "D/CameraId" }, "additionalProperties": { "type": "string" },
            "description": "camera id -> relativePath of the chosen file" } } } },
      "representative": { "type": ["string", "null"] }, "thumbnail": { "type": ["string", "null"] },
      "metadata": { "oneOf": [ { "type": "null" }, { "$ref": "D/Metadata" } ] } } },
  "case": { "type": "object", "properties": {
    "fn": { "const": "buildTeslaCamLibrary" },
    "input": { "type": "object", "required": ["files"], "additionalProperties": false,
      "properties": { "files": { "type": "array", "items": { "$ref": "#/$defs/InputFile" } },
        "generate": { "type": "object", "required": ["count", "pathPattern"], "additionalProperties": false,
          "properties": { "count": { "type": "integer" }, "pathPattern": { "type": "string" }, "size": { "type": "integer", "default": 14 } } } } },
    "expected": { "oneOf": [
      { "type": "object", "required": ["events", "eventsByCategory", "warnings"], "additionalProperties": false,
        "properties": { "events": { "type": "array", "items": { "$ref": "#/$defs/LibraryEvent" } },
          "eventsByCategory": { "type": "object", "required": ["recent", "saved", "sentry"], "additionalProperties": false,
            "properties": { "recent": { "type": "array", "items": { "type": "string" } }, "saved": { "type": "array", "items": { "type": "string" } },
              "sentry": { "type": "array", "items": { "type": "string" } } } },
          "warnings": { "type": "array", "items": { "$ref": "D/Warning" } },
          "progress": { "type": "array", "items": { "$ref": "D/Progress" } } } },
      { "type": "object", "required": ["error", "kind"], "additionalProperties": false,
        "properties": { "error": { "const": "scan-limit" }, "kind": { "$ref": "D/ScanLimitKind" } } } ] } } } }
```

Cases: every case in `build-library.test.ts` and `scan-limits.test.ts`; helper precedence; helpers in `RecentClips`;
clip directly in `SavedClips/`; duplicate `(timestamp, camera)`; `EncryptedClips` first in path; nested category
names; event with only helpers (dropped, metadata not read); invalid `event.json` (warning); midnight and year-boundary
Recent sessions; 90 s vs 91 s gaps; same-timestamp ties across and within categories; the real e2e fixture tree.

#### 6.3.6 `event-format-filters.json` — §1.13, §1.15

```json
{ "Filters": { "type": "object", "required": ["category", "dateKey", "query"], "additionalProperties": false,
    "properties": { "category": { "$ref": "D/CategoryFilter" }, "dateKey": { "type": ["string", "null"] }, "query": { "type": "string" } } },
  "case": { "oneOf": [
    { "type": "object", "properties": { "fn": { "enum": ["eventDate", "eventClock", "eventTime"] },
      "input": { "type": "object", "required": ["timestamp"], "properties": { "timestamp": { "type": "string" } } }, "expected": { "type": "string" } } },
    { "type": "object", "properties": { "fn": { "const": "categoryLabel" },
      "input": { "type": "object", "required": ["category"], "properties": { "category": { "$ref": "D/ClipCategory" } } }, "expected": { "type": "string" } } },
    { "type": "object", "properties": { "fn": { "const": "eventMatchesFilters" },
      "input": { "type": "object", "required": ["event", "filters"], "properties": { "event": { "$ref": "D/EventFixture" }, "filters": { "$ref": "#/$defs/Filters" } } },
      "expected": { "type": "boolean" } } },
    { "type": "object", "properties": { "fn": { "const": "orderedMatchingEvents" },
      "input": { "type": "object", "required": ["events", "filters"], "properties": { "events": { "type": "array", "items": { "$ref": "D/EventFixture" } }, "filters": { "$ref": "#/$defs/Filters" } } },
      "expected": { "type": "array", "items": { "type": "string" }, "description": "event ids in order" } } } ] } }
```

Cases: queries `20:45`, ` SENTRY `, `セントリー`, `2026-08-20 20`, empty/space-only; date prefix; stable ties.

#### 6.3.7 `event-summary.json` — §1.14

```json
{ "case": { "oneOf": [
    { "type": "object", "properties": { "fn": { "const": "cameraAvailability" },
      "input": { "type": "object", "required": ["event"], "properties": { "event": { "$ref": "D/EventFixture" } } },
      "expected": { "type": "object", "required": ["available", "total", "missing"], "additionalProperties": false,
        "properties": { "available": { "type": "integer" }, "total": { "const": 6 }, "missing": { "type": "array", "items": { "$ref": "D/CameraId" } } } } } },
    { "type": "object", "properties": { "fn": { "enum": ["cameraAvailabilityLabel", "missingCameraLabel", "eventDurationLabel", "segmentCountLabel"] },
      "input": { "type": "object", "required": ["event"], "properties": { "event": { "$ref": "D/EventFixture" } } },
      "expected": { "type": ["string", "null"] } } },
    { "type": "object", "properties": { "fn": { "const": "formatDuration" },
      "input": { "type": "object", "required": ["seconds"], "properties": { "seconds": { "type": "number" } } },
      "expected": { "type": "string" } } } ] } }
```

#### 6.3.8 `event-detection.json` — §1.16

```json
{ "case": { "type": "object", "properties": {
    "fn": { "enum": ["detectionSeconds", "sentryInitialElapsedSeconds", "sentryPlaybackStartLabel"] },
    "input": { "type": "object", "required": ["event"], "properties": { "event": { "$ref": "D/EventFixture" } } },
    "expected": { "type": ["number", "string", "null"] } } } }
```

Cases: all of `event-detection.test.ts`; millisecond detections (`12:00:05.900` → 5); detection < 15 s
(pre-roll 0, label = first segment start); Saved/Recent with valid detection (`detectionSeconds` non-null, pre-roll 0,
label null).

#### 6.3.9 `playback-timeline.json` — §1.17, §2.10.8, §2.11

```json
{ "case": { "oneOf": [
    { "type": "object", "properties": { "fn": { "const": "eventDuration" },
      "input": { "type": "object", "required": ["segmentCount"], "properties": { "segmentCount": { "type": "integer", "minimum": 0 } } },
      "expected": { "type": "number" } } },
    { "type": "object", "properties": { "fn": { "const": "locatePlaybackTime" },
      "input": { "type": "object", "required": ["segmentCount", "elapsedSeconds"],
        "properties": { "segmentCount": { "type": "integer", "minimum": 0 }, "elapsedSeconds": { "$ref": "D/Num" } } },
      "expected": { "type": "object", "required": ["segmentIndex", "offsetSeconds"], "additionalProperties": false,
        "properties": { "segmentIndex": { "type": "integer" }, "offsetSeconds": { "type": "number" } } } } },
    { "type": "object", "properties": { "fn": { "enum": ["transportTimeLabel", "exportClockLabel"] },
      "description": "(extract) PlaybackControls.formatTime and CompositeVideoExport.clockLabel",
      "input": { "type": "object", "required": ["seconds"], "properties": { "seconds": { "type": "number" } } },
      "expected": { "type": "string" } } },
    { "type": "object", "properties": { "fn": { "const": "detectionMarkerPercent" },
      "description": "(extract) PlaybackControls detectionPercent",
      "input": { "type": "object", "required": ["durationSeconds", "detectionSeconds"],
        "properties": { "durationSeconds": { "type": "number" }, "detectionSeconds": { "type": ["number", "null"] } } },
      "expected": { "type": ["number", "null"] } } } ] } }
```

#### 6.3.10 `playback-machine.json` — §1.18 (pure state machine)

```json
{ "Step": { "type": "object", "required": ["do"], "additionalProperties": false,
    "properties": {
      "do": { "enum": ["register", "unregister", "media", "play", "pause", "seek", "skip", "setRate", "setPreferred", "setMediaTime", "setPlayOutcome", "settlePlay"] },
      "camera": { "$ref": "D/CameraId" },
      "element": { "type": "integer", "minimum": 0, "description": "player instance number, to test stale registrations" },
      "event": { "enum": ["loading", "ready", "unavailable", "timeupdate", "ended"] },
      "value": { "type": ["number", "null"] },
      "preferred": { "oneOf": [ { "$ref": "D/CameraId" }, { "type": "null" } ] },
      "result": { "enum": ["fulfilled", "rejected", "pending"],
        "description": "register/setPlayOutcome: outcome of future play calls of this element (default fulfilled); settlePlay: fulfilled or rejected for the oldest pending call" },
      "expect": { "$ref": "#/$defs/Snapshot" } } },
  "Snapshot": { "type": "object", "additionalProperties": false,
    "properties": { "playing": { "type": "boolean" }, "elapsedSeconds": { "type": "number" }, "segmentIndex": { "type": "integer" },
      "segmentOffsetSeconds": { "type": "number" }, "rate": { "type": "number" }, "message": { "$ref": "D/Message" },
      "sourceCamera": { "oneOf": [ { "$ref": "D/CameraId" }, { "type": "null" } ] },
      "mediaTime": { "type": "object", "propertyNames": { "$ref": "D/CameraId" }, "additionalProperties": { "type": "number" } },
      "playbackRate": { "type": "object", "propertyNames": { "$ref": "D/CameraId" }, "additionalProperties": { "type": "number" } },
      "playCalls": { "type": "object", "propertyNames": { "$ref": "D/CameraId" }, "additionalProperties": { "type": "integer" } },
      "pauseCalled": { "type": "array", "items": { "$ref": "D/CameraId" } } } },
  "case": { "oneOf": [
    { "type": "object", "properties": { "fn": { "const": "resolvePlayback" },
      "input": { "type": "object", "required": ["intended", "registered", "lifecycles", "attempts", "successful"],
        "properties": { "intended": { "type": "boolean" }, "registered": { "type": "array", "items": { "$ref": "D/CameraId" } },
          "lifecycles": { "type": "object", "additionalProperties": { "$ref": "D/Lifecycle" } },
          "attempts": { "type": "object", "additionalProperties": { "$ref": "D/AttemptStatus" } },
          "successful": { "type": "array", "items": { "$ref": "D/CameraId" } } } },
      "expected": { "enum": ["paused", "playing", "waiting", "failed"] } } },
    { "type": "object", "properties": { "fn": { "const": "electPlaybackSource" },
      "input": { "type": "object", "required": ["ready", "preferred"],
        "properties": { "ready": { "type": "array", "items": { "$ref": "D/CameraId" } }, "preferred": { "oneOf": [ { "$ref": "D/CameraId" }, { "type": "null" } ] } } },
      "expected": { "oneOf": [ { "$ref": "D/CameraId" }, { "type": "null" } ] } } },
    { "type": "object", "properties": { "fn": { "const": "playbackScenario" },
      "input": { "type": "object", "required": ["segmentCount", "steps"],
        "properties": { "segmentCount": { "type": "integer", "minimum": 0 }, "initialElapsedSeconds": { "type": "number", "default": 0 },
          "preferred": { "oneOf": [ { "$ref": "D/CameraId" }, { "type": "null" } ], "default": null },
          "steps": { "type": "array", "items": { "$ref": "#/$defs/Step" } } } },
      "expected": { "type": "null", "description": "assertions live in steps[].expect" } } } ] } }
```

Scenario semantics: fake players record media time, rate, play and pause calls. Each play call settles immediately
(before the next step) with the element's configured outcome (`register`/`setPlayOutcome`, default `fulfilled`);
outcome `pending` keeps the call open until a `settlePlay` step; calls still pending at the end never settle.
`expect` is checked after the step and after all immediate settlements. Cases: every test in `use-synchronized-playback*.test.tsx` (initial clamped position, isolated
rejection, all rejected, stale unregister, seek/skip/rate/pause, pending intent across seek/skip/rate, old-source
settlement, autoplay once, no replay, pause cancels autoplay, rejected autoplay message, drift 0.25/0.26, segment
entry at 60, re-election on failure, final ended pause, preferred change, repeated ready, segment-transition autoplay,
partial rejection, all terminal).

#### 6.3.11 `day-timeline.json` — §1.20

```json
{ "Marker": { "type": "object", "required": ["eventId", "category", "dateKey", "timeLabel", "secondsOfDay", "positionPercent"], "additionalProperties": false,
    "properties": { "eventId": { "type": "string" }, "category": { "$ref": "D/ClipCategory" }, "dateKey": { "$ref": "D/DateKey" },
      "timeLabel": { "type": "string" }, "secondsOfDay": { "type": "integer" }, "positionPercent": { "type": "number" } } },
  "Day": { "type": "object", "required": ["dateKey", "eventIds"], "additionalProperties": false,
    "properties": { "dateKey": { "$ref": "D/DateKey" }, "eventIds": { "type": "array", "items": { "type": "string" } } } },
  "Cluster": { "type": "object", "additionalProperties": false,
    "required": ["id", "count", "eventIds", "startTimeLabel", "endTimeLabel", "startSecondsOfDay", "endSecondsOfDay", "durationSeconds", "startPositionPercent", "endPositionPercent"],
    "properties": { "id": { "type": "string" }, "count": { "type": "integer" }, "eventIds": { "type": "array", "items": { "type": "string" } },
      "startTimeLabel": { "type": "string" }, "endTimeLabel": { "type": "string" }, "startSecondsOfDay": { "type": "integer" },
      "endSecondsOfDay": { "type": "integer" }, "durationSeconds": { "type": "integer" },
      "startPositionPercent": { "type": "number" }, "endPositionPercent": { "type": "number" } } },
  "case": { "oneOf": [
    { "type": "object", "properties": { "fn": { "const": "selectTimelineDate" },
      "input": { "type": "object", "required": ["events", "selectedEventId", "filterDateKey"],
        "properties": { "events": { "type": "array", "items": { "$ref": "D/EventFixture" } }, "selectedEventId": { "type": ["string", "null"] },
          "filterDateKey": { "type": ["string", "null"] } } },
      "expected": { "type": ["string", "null"] } } },
    { "type": "object", "properties": { "fn": { "const": "buildDayTimelineMarkers" },
      "input": { "type": "object", "required": ["events", "dateKey"],
        "properties": { "events": { "type": "array", "items": { "$ref": "D/EventFixture" } }, "dateKey": { "type": ["string", "null"] } } },
      "expected": { "type": "array", "items": { "$ref": "#/$defs/Marker" } } } },
    { "type": "object", "properties": { "fn": { "const": "buildTimelineDays" },
      "input": { "type": "object", "required": ["events"], "properties": { "events": { "type": "array", "items": { "$ref": "D/EventFixture" } } } },
      "expected": { "type": "array", "items": { "$ref": "#/$defs/Day" } } } },
    { "type": "object", "properties": { "fn": { "const": "adjacentTimelineDays" },
      "input": { "type": "object", "required": ["days", "currentDateKey"],
        "properties": { "days": { "type": "array", "items": { "$ref": "#/$defs/Day" } }, "currentDateKey": { "type": ["string", "null"] } } },
      "expected": { "type": "object", "required": ["previous", "next"], "additionalProperties": false,
        "properties": { "previous": { "oneOf": [ { "type": "null" }, { "$ref": "#/$defs/Day" } ] }, "next": { "oneOf": [ { "type": "null" }, { "$ref": "#/$defs/Day" } ] } } } } },
    { "type": "object", "properties": { "fn": { "const": "buildDayTimelineClusters" },
      "input": { "type": "object", "required": ["events", "dateKey", "trackWidthPixels"],
        "properties": { "events": { "type": "array", "items": { "$ref": "D/EventFixture" } }, "dateKey": { "type": ["string", "null"] },
          "trackWidthPixels": { "$ref": "D/Num" } } },
      "expected": { "type": "array", "items": { "$ref": "#/$defs/Cluster" } } } },
    { "type": "object", "properties": { "fn": { "const": "timelineLabels" },
      "description": "(extract) DayTimelineRail/DayTimeline/TimelineEventButton label builders",
      "input": { "type": "object", "required": ["kind"], "additionalProperties": false,
        "properties": { "kind": { "enum": ["clusterRange", "clusterGroup", "clusterTrigger", "markerButton", "dayButton", "markerSummary"] },
          "startTimeLabel": { "type": "string" }, "endTimeLabel": { "type": "string" }, "count": { "type": "integer" },
          "expanded": { "type": "boolean" }, "dateKey": { "type": ["string", "null"] }, "timeLabel": { "type": "string" },
          "category": { "$ref": "D/ClipCategory" }, "direction": { "enum": ["previous", "next"] }, "present": { "type": "boolean" } } },
      "expected": { "type": "string" } } } ] } }
```

`adjacentTimelineDays` encodes "undefined" as `null`. Cases: all of `day-timeline.test.ts`; the 640-pt threshold at
5 939 / 5 940 s; chained clustering (three markers 5 000 s apart form one cluster); non-finite and zero widths;
`timelineLabels` examples `12:00–13:00`, `12:00–13:00・2件を開く`, `2026-08-24 20:40:00 保存済み`,
`前の記録日 2026-08-19`, `00:00:00 セントリー`, `なし`.

#### 6.3.12 `app-state.json` — §1.21

```json
{ "Action": { "type": "object", "required": ["kind"], "additionalProperties": false,
    "properties": { "kind": { "enum": ["scan-started", "scan-progressed", "scan-cancelled", "scan-denied", "scan-invalid", "scan-failed",
        "library-ready", "category-filtered", "date-filtered", "query-filtered", "event-selected"] },
      "progress": { "$ref": "D/Progress" }, "folderName": { "type": "string" },
      "events": { "type": "array", "items": { "$ref": "D/EventFixture" }, "description": "library-ready: library.events; eventsByCategory is derived" },
      "category": { "$ref": "D/CategoryFilter" }, "dateKey": { "type": ["string", "null"] }, "query": { "type": "string" },
      "eventId": { "type": "string" } } },
  "State": { "type": "object", "required": ["kind"], "additionalProperties": false,
    "properties": { "kind": { "enum": ["idle", "scanning", "error", "ready"] }, "reason": { "enum": ["denied", "invalid", "failed"] },
      "progress": { "$ref": "D/Progress" }, "folderName": { "type": "string" },
      "filters": { "type": "object", "required": ["category", "dateKey", "query"], "properties": { "category": { "$ref": "D/CategoryFilter" },
        "dateKey": { "type": ["string", "null"] }, "query": { "type": "string" } } },
      "selectedEventId": { "type": ["string", "null"] }, "eventIds": { "type": "array", "items": { "type": "string" } } } },
  "case": { "type": "object", "properties": {
    "fn": { "const": "appReducerScenario" },
    "input": { "type": "object", "required": ["steps"], "properties": { "steps": { "type": "array",
      "items": { "type": "object", "required": ["action", "expect"], "additionalProperties": false,
        "properties": { "action": { "$ref": "#/$defs/Action" }, "expect": { "$ref": "#/$defs/State" } } } } } },
    "expected": { "type": "null" } } } }
```

Starts from the initial state. Cases: all of `app-state.test.ts`, cancel from `ready`, progress ignored outside
scanning, filter actions ignored outside `ready`.

#### 6.3.13 `composite-layout.json` — §3.2–§3.3

```json
{ "case": { "oneOf": [
    { "type": "object", "properties": { "fn": { "const": "buildCompositeLayout" }, "input": { "type": "object", "maxProperties": 0 },
      "expected": { "type": "object", "required": ["width", "height", "vehicle", "cameras"], "additionalProperties": false,
        "properties": { "width": { "const": 1280 }, "height": { "const": 720 }, "vehicle": { "$ref": "D/Rect" },
          "cameras": { "type": "array", "minItems": 6, "maxItems": 6, "items": { "type": "object", "required": ["cameraId", "label", "frame"],
            "additionalProperties": false, "properties": { "cameraId": { "$ref": "D/CameraId" }, "label": { "type": "string" }, "frame": { "$ref": "D/Rect" } } } } } } } },
    { "type": "object", "properties": { "fn": { "const": "containRect" },
      "input": { "type": "object", "required": ["source", "destination"],
        "properties": { "source": { "type": "object", "required": ["width", "height"], "properties": { "width": { "type": "number" }, "height": { "type": "number" } } },
          "destination": { "$ref": "D/Rect" } } },
      "expected": { "$ref": "D/Rect" } } },
    { "type": "object", "properties": { "fn": { "const": "slotLabel" },
      "description": "(extract) label text and geometry of drawCompositeFrame for a measured text advance width",
      "input": { "type": "object", "required": ["cameraId", "playable", "textAdvanceWidth"],
        "properties": { "cameraId": { "$ref": "D/CameraId" }, "playable": { "type": "boolean" }, "textAdvanceWidth": { "type": "number", "minimum": 0 } } },
      "expected": { "type": "object", "required": ["text", "scrim", "textOrigin"], "additionalProperties": false,
        "properties": { "text": { "type": "string" }, "scrim": { "$ref": "D/Rect" },
          "textOrigin": { "type": "object", "required": ["x", "y"], "properties": { "x": { "type": "number" }, "y": { "type": "number" } } } } } } } ] } }
```

Cases: the layout table; `containRect` examples of §3.3 and the vehicle (360 × 720 → 594/260/92/184);
`slotLabel({back, false, 56})` → text `リア・記録なし`, scrim (408, 500, 72, 24), origin (416, 517); zero-size source.

#### 6.3.14 `composite-export-plan.json` — §3.4, §3.6 (new pure planner)

The web currently interleaves this logic in `tryExportCompositeWithWebCodecs`; it SHOULD be extracted as
`planCompositeExport(segments)` and both encoders SHOULD drive their loops from the plan.

```json
{ "case": { "type": "object", "properties": {
    "fn": { "const": "planCompositeExport" },
    "input": { "type": "object", "required": ["segments"], "additionalProperties": false,
      "properties": { "segments": { "type": "array", "items": { "type": "object", "required": ["cameras"], "additionalProperties": false,
        "properties": { "cameras": { "type": "object", "propertyNames": { "$ref": "D/CameraId" },
          "additionalProperties": { "type": ["number", "null"], "description": "decodable duration in seconds; null = present but not decodable" } } } } } } },
    "expected": { "oneOf": [
      { "type": "object", "required": ["segments", "totalFrames", "totalDurationSeconds", "progress"], "additionalProperties": false,
        "properties": {
          "segments": { "type": "array", "items": { "type": "object", "additionalProperties": false,
            "required": ["skipped", "durationSeconds", "frameCount", "startSeconds", "cameraFrameCounts"],
            "properties": { "skipped": { "type": "boolean" }, "durationSeconds": { "type": "number" }, "frameCount": { "type": "integer" },
              "startSeconds": { "type": "number" },
              "cameraFrameCounts": { "type": "object", "propertyNames": { "$ref": "D/CameraId" }, "additionalProperties": { "type": "integer" } } } } },
          "totalFrames": { "type": "integer", "minimum": 1 }, "totalDurationSeconds": { "type": "number" },
          "progress": { "type": "array", "items": { "type": "number" }, "description": "every reported value in order, starting with 0 and ending with 1" } } },
      { "type": "object", "required": ["error"], "additionalProperties": false, "properties": { "error": { "const": "no-playable-media" } } } ] } } } }
```

Cases: one 2 s camera (60 frames, progress `[0, 1/60, 31/60, 1, 1]`); five 2 s segments (300 frames, 10 s);
a 0-camera middle segment (skipped, progress jumps); mixed camera durations (20 s and 2 s → per-camera frame counts
600 and 60); undecodable camera; all undecodable → `no-playable-media`; 59.97 s → `ceil` frame count 1 800 (not 1 799).

#### 6.3.15 `export-state.json` — §2.11

```json
{ "case": { "oneOf": [
    { "type": "object", "properties": { "fn": { "const": "exportReducerScenario" },
      "input": { "type": "object", "required": ["steps"], "properties": { "steps": { "type": "array", "items": { "type": "object",
        "required": ["action", "expect"], "additionalProperties": false,
        "properties": {
          "action": { "type": "object", "required": ["kind"], "properties": {
            "kind": { "enum": ["dialog-opened", "dialog-closed", "started", "progressed", "completed", "failed", "cancelled", "reset"] },
            "format": { "type": "object", "required": ["label", "extension"], "properties": { "label": { "enum": ["MP4", "WebM"] },
              "extension": { "enum": ["mp4", "webm"] }, "fallbackLabel": { "enum": ["MP4", "WebM"] } } },
            "progress": { "type": "number" }, "extension": { "enum": ["mp4", "webm"] }, "error": { "type": "string" } } },
          "expect": { "type": "object", "additionalProperties": false, "properties": {
            "status": { "enum": ["idle", "exporting", "complete", "error"] }, "dialogOpen": { "type": "boolean" },
            "notice": { "type": ["string", "null"] }, "progress": { "type": "number" }, "error": { "type": "string" },
            "extension": { "enum": ["mp4", "webm"] } } } } } } } },
      "expected": { "type": "null" } } },
    { "type": "object", "properties": { "fn": { "enum": ["exportUnavailableReason", "exportCompletionText", "exportSaveLabel", "exportFormatValue", "exportFileName", "exportProgressText"] },
      "description": "(extract) copy helpers of CompositeVideoExport / CompositeExportDialog",
      "input": { "type": "object", "properties": { "available": { "type": "integer" }, "encoderSupported": { "type": "boolean" },
        "extension": { "enum": ["mp4", "webm"] }, "label": { "enum": ["MP4", "WebM"] }, "fallbackLabel": { "enum": ["MP4", "WebM"] },
        "timestamp": { "type": "string" }, "progress": { "type": "number" }, "durationSeconds": { "type": "number" } } },
      "expected": { "type": ["string", "null"] } } } ] } }
```

#### 6.3.16 `theater-shortcuts.json` — §2.10.10

```json
{ "case": { "type": "object", "properties": {
    "fn": { "const": "theaterShortcut" },
    "input": { "type": "object", "required": ["key", "focus", "expandedCamera", "playing"], "additionalProperties": false,
      "properties": { "key": { "type": "string", "description": "KeyboardEvent.key value, e.g. ' ', 'ArrowLeft', '6', 'Escape'" },
        "modifiers": { "type": "array", "items": { "enum": ["cmd", "ctrl", "alt", "shift"] }, "uniqueItems": true },
        "focus": { "enum": ["none", "button", "text", "select", "editable"] },
        "expandedCamera": { "oneOf": [ { "$ref": "D/CameraId" }, { "type": "null" } ] }, "playing": { "type": "boolean" },
        "dialogOpen": { "type": "boolean", "default": false } } },
    "expected": { "type": "object", "required": ["action", "consumed"], "additionalProperties": false,
      "properties": { "action": { "enum": ["none", "collapse", "play", "pause", "skip", "toggleCamera"] },
        "skipSeconds": { "type": "number" }, "camera": { "$ref": "D/CameraId" }, "consumed": { "type": "boolean" } } } } } }
```

Cases with `modifiers` or `dialogOpen` carry `"platforms": ["mac"]` (§8 A-21).

#### 6.3.17 `copy-format.json` — templates of §5

```json
{ "case": { "type": "object", "properties": {
    "fn": { "enum": ["scanProgressText", "warningText", "warningsSummaryCount", "sourceEventCount", "listCount", "missingCameraText"] },
    "input": { "type": "object", "properties": { "processed": { "type": "integer" }, "total": { "type": "integer" },
      "kind": { "$ref": "D/WarningKind" }, "count": { "type": "integer" }, "cameras": { "type": "array", "items": { "$ref": "D/CameraId" } } } },
    "expected": { "type": "string" } } } }
```

Cases: `{0,0}` → `フォルダを読み込んでいます。`; `{250,500}` → `250 / 500 ファイルを確認中です。`;
`{12345,12345}` → `12,345 ファイルを確認中です。`; each warning kind; `{count: 1234}` warning → `1234` (no grouping).

### 6.4 Resources

`strings/ja.json` (schema `strings.schema.json`):

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "strings.schema.json",
  "type": "object", "required": ["contract", "schemaVersion", "strings"], "additionalProperties": false,
  "properties": {
    "contract": { "const": "teslacam-viewer" }, "schemaVersion": { "const": 1 },
    "strings": { "type": "object",
      "propertyNames": { "pattern": "^[a-z][A-Za-z0-9]*(\\.[a-z][A-Za-z0-9]*)+$" },
      "additionalProperties": { "type": "object", "required": ["text", "role", "platforms"], "additionalProperties": false,
        "properties": {
          "text": { "type": "string", "description": "verbatim; placeholders as {name}; \\n for line breaks" },
          "placeholders": { "type": "array", "items": { "type": "string" }, "uniqueItems": true },
          "role": { "enum": ["visible", "accessibility", "error", "rendered-in-video"] },
          "platforms": { "type": "array", "items": { "enum": ["web", "mac"] }, "minItems": 1, "uniqueItems": true },
          "browserWorded": { "type": "boolean", "default": false } } } } }
}
```

IDs and texts are exactly those of §5. A web test SHOULD assert that each catalog text appears in the web source (or,
better, the web imports the catalog); the Mac embeds the catalog with `include_str!` and a test asserts that every
`platforms: ["…mac…"]` entry is referenced.

`tokens/design-tokens.json` (schema `design-tokens.schema.json`): groups `color` (§4.1, values as `#rrggbb` or
`rgba(r, g, b, a)`), `scopedColor` (§4.2, `{scope: {token: value}}`), `font` (§4.3 levels as
`{size, weight, lineHeight, tracking}`, `size` may be `{clampMin, viewportFraction, clampMax}`), `space` (§4.5),
`radius` and `size` (§4.6), `composite` (§4.4). Schema: every leaf is either a number (points) or a color string
matching `^(#[0-9a-f]{6}|rgba\(\d{1,3}, \d{1,3}, \d{1,3}, (0|1|0?\.\d+)\))$`. A web test SHOULD compare
`tokens.css` against this file; the Mac theme module is generated from it (or tested against it).

---

## 7. Acceptance checklist

Each item is Given / When / Then and names its web source. "Fixture *full*" etc. refer to the synthetic trees of
`tests/e2e/support/viewer-fixtures.ts` (the Mac test suite SHOULD build the same trees on disk from
`apps/web/tests/e2e/fixtures/**` media); "contract" means the item is fully covered by §6 fixtures.

Fixture trees (all 2 s, 320 × 180, 24 fps H.264 clips unless noted):

- *full*: `TeslaCam/Noise/` (253 `synthetic-*.txt`), `RecentClips/` (6 cameras at `2026-08-20_20-45-00`),
  `SavedClips/event-saved/` (6 cameras at `2026-08-20_20-40-00` + `event.mp4`), `SentryClips/event-sentry/`
  (6 cameras at `2026-08-20_20-35-00` using the 20 s clip + `event.json` with detection `2026-08-20T20:35:30`,
  reason `sentry_aware_object_detection`, city/street/lat/lon `SYNTHETIC_PRIVATE_*`).
- *history*: *full* plus `SavedClips/event-saved-previous/` (6 cameras at `2026-08-19_19-30-00` + `event.mp4`).
- *missing*: *full* with the Sentry event lacking `right_repeater`.
- *export*: `SavedClips/event-export/` with 6 cameras at `20-10-00`, `20-10-02`, `20-10-04`, `20-10-06`, `20-10-08`.
- *corrupt*: `SavedClips/event-corrupt/` with 6 four-byte clips at `20-30-00` + a valid `event.mp4`.
- *wrong*: `SyntheticFolder/notes.txt`.

### 7.1 Ingestion and library

- [ ] **AT-ING-01** Given the real directory `apps/web/tests/e2e/fixtures/TeslaCam`, when it is selected, then the
  workspace opens with `すべて` 1, `最近の走行` 1, `保存済み` 0, `セントリー` 0 and one warning
  `対応外のファイル 1 件を除外しました。` (`error-states.spec.ts` fallback test; the Sentry directory has no valid clip).
- [ ] **AT-ING-02** (contract) Given Saved clips at `14-30-45` (front, back) and `14-31-45` (front) plus helpers in
  `saved-a` and one clip in `saved-b` at `15-30-45`, when built, then 2 Saved events, newest `15-30-45` first; the older
  has 2 segments, its helpers attached, and metadata `{timestamp, reason: 'manual_save'}` without unknown fields.
- [ ] **AT-ING-03** (contract) Given six cameras with one timestamp in one directory, then one segment holds all six.
- [ ] **AT-ING-04** (contract) Given two Sentry directories with identical clip timestamps, then two events in
  first-seen order with ids `…-1` and `…-2`.
- [ ] **AT-ING-05** (contract) Given a 0-byte clip, a `-cabin` clip, an unknown `.bin` and an `EncryptedClips` clip,
  then no events and warnings `[empty-media-file 1, unknown-file 2, encrypted-file 1]`; warnings contain no path or name.
- [ ] **AT-ING-06** (contract) Given Recent clips at `23:59:30`, `00:00:50` (+1 day), `00:02:21`, then two sessions:
  `00:02:21` (1 segment) and `23:59:30` (2 segments).
- [ ] **AT-ING-07** (contract) Given 251 unknown files, then progress `{250, 251}` then `{251, 251}` and one warning
  `unknown-file 251`; given a walk of 251 entries, progress `{250, 250}` then `{251, 251}`.
- [ ] **AT-ING-08** (contract) Given a root `TeslaCam` with `SavedClips/saved-a/{clip, .DS_Store}` and
  `SavedClips/.ignored/{clip}`, then exactly one file `TeslaCam/SavedClips/saved-a/<clip>`; given a root named
  `RecentClips`/`SavedClips`/`SentryClips` with a clip directly inside, then the path is `<root>/<clip>`.
- [ ] **AT-ING-09** (contract) Given a 0-byte Recent clip, then no events and warning `empty-media-file 1`, and the
  app shows the `invalid` error screen.
- [ ] **AT-ING-10** Given a directory chain of depth 9, when selected, then the `failed` error screen
  (`フォルダの読み取りを完了できませんでした。`) appears (`scan-limits.test.ts`, `use-directory-scan.test.tsx`).
- [ ] **AT-ING-11** Given more than 100 000 entries, then the `failed` screen.
- [ ] **AT-ING-12** (contract) Given `event.json` larger than 1 MiB (declared or actual), then the `failed` screen.
- [ ] **AT-ING-13** (contract) Given any of the seven known fields longer than 2048 UTF-16 units, then the `failed`
  screen; exactly 2048 is accepted.
- [ ] **AT-ING-14** (contract) Given `event.json` values `null`, `[]`, `42`, `{"timestamp": 42}`,
  `{"timestamp": "…", "reason": 42}`, then that event has no metadata and the warning
  `読み取れないイベント情報 1 件を除外しました。` is listed.
- [ ] **AT-ING-15** (contract) Filename grammar cases of §6.3.1.
- [ ] **AT-ING-16** Given a slow first scan and a second folder chosen before it ends, then only the second scan's
  progress and result are applied (`use-directory-scan.test.tsx`).

### 7.2 Welcome, scanning, errors

- [ ] **AT-WEL-01** Given launch, then the welcome screen shows `TeslaCam Viewer`, `SSDのTeslaCamフォルダを開く` (with
  `フォルダを開く` on one line), the description, the single primary button `TeslaCamフォルダを選ぶ`, `選ぶもの` and
  `表示されるもの` (`App.test.tsx`, `layout.spec.ts`).
- [ ] **AT-WEL-02** Given macOS in Dark Mode, when the app runs, then all surfaces use the light tokens: background
  luminance > 0.8 and body text contrast ≥ 4.5:1 (`theme.spec.ts`).
- [ ] **AT-SCAN-01** (contract) Progress text: `{0,0}` → `フォルダを読み込んでいます。`; `{250,500}` →
  `250 / 500 ファイルを確認中です。`; `{250,250}` → `250 ファイルを確認中です。` (`App.test.tsx`). With fixture *full*
  the `250 ファイルを確認中です。` state is observable during the scan (`layout.spec.ts`).
- [ ] **AT-ERR-01** Given fixture *wrong*, when selected, then `TeslaCamフォルダを確認できませんでした。`.
- [ ] **AT-ERR-02** Given a root whose read permission is refused, then `フォルダへのアクセスが許可されませんでした。`
  and `もう一度選ぶ` reopens the picker (`App.directory-picker.test.tsx`).
- [ ] **AT-ERR-03** Given an unexpected picker or I/O failure, then `フォルダの読み取りを完了できませんでした。`.
- [ ] **AT-ERR-04** Given the open panel is cancelled (from welcome or from a loaded workspace), then the welcome
  heading `SSDのTeslaCamフォルダを開く` is shown (`error-states.spec.ts`; §8 A-1).
- [ ] **AT-ERR-05** Given a loaded workspace with an encrypted-file warning, when an invalid folder is chosen, then
  the `invalid` screen; when `もう一度選ぶ` and a valid folder are chosen, then the workspace returns
  (`App.directory-picker.test.tsx`).
- [ ] **AT-ERR-06** Given any error during scanning, then no runtime error or crash is logged.

### 7.3 Navigator, sidebar and event list

- [ ] **AT-NAV-01** Given fixture *full*, when loaded, then categories read `すべて 3`, `最近の走行 1`, `保存済み 1`,
  `セントリー 1`, and choosing each shows `3件`, `1件`, `1件`, `1件` in the list header (`interactions.spec.ts`).
- [ ] **AT-NAV-02** (contract) Given a selected Sentry event, when category, date and query filters still match, the
  selection stays; when the category changes to `saved`, the selection is cleared and the placeholder appears
  (`app-state.test.ts`).
- [ ] **AT-NAV-03** (contract) Matching events are listed newest first (`app-state.test.ts`).
- [ ] **AT-NAV-04** (contract) Queries `20:45`, `sentry`, `セントリー` match the Sentry event at `20:45:00`; a query
  with surrounding spaces behaves like the trimmed query.
- [ ] **AT-NAV-05** Given warnings, then the `読み込みの注意` disclosure is collapsed, shows the number of warning
  kinds (`1件` for *full*), and expanding reveals `対応外のファイル 253 件を除外しました。` (`App.directory-picker.test.tsx`).
- [ ] **AT-NAV-06** Given fixture *full*, then the folder name `TeslaCam` appears exactly once (sidebar) and the
  sidebar shows `3 件のイベント` (`layout.spec.ts`).
- [ ] **AT-NAV-07** Given the date field is set to `2026-08-20` and then cleared, then the list is filtered and then
  unfiltered.
- [ ] **AT-LIST-01** Given a Sentry event with only `front` and one segment, then its row shows `2026-08-20`,
  `20:45:00`, `1分`, `1セグメント`, `1/6 カメラ`, and `リア、左ピラー、左リピーター、右ピラー、右リピーターなし`, and never
  `台のカメラがありません` (`EventList.test.tsx`).
- [ ] **AT-LIST-02** Given that row is selected and has a valid detection, then `検知の15秒前から開始` appears only on it.
- [ ] **AT-LIST-03** Given no matching events, then `この条件に一致するイベントはありません。`.
- [ ] **AT-LIST-04** Given an event with `thumb.png`, then the thumbnail shows and its image handle is released when
  the row disappears; given a failing or missing thumbnail, then no image frame is drawn; fixture *full* shows no
  thumbnails (`EventList.thumbnail.test.tsx`, `layout.spec.ts`).
- [ ] **AT-LIST-05** Given a full first segment and a front-only second segment, then the row shows `1/6 カメラ`.
- [ ] **AT-LIST-06** The category badge (`セントリー` etc.) never wraps, at the navigator width of 320 pt.

### 7.4 Selection and autoplay

- [ ] **AT-SEL-01** Given a Sentry event with segments `12:00:00`, `12:01:00` and detection `12:01:20`, when selected,
  then the position slider shows 65, every camera's media time is 5 s once ready, and playback starts on its own
  (`一時停止` visible) (`Theater.test.tsx`, `App.selection.test.tsx`).
- [ ] **AT-SEL-02** Given that event is playing at media time 9 s, when it is re-selected from the timeline (after
  opening `12:00–13:00・2件を開く`) or from the list, then the media time stays 9 s.
- [ ] **AT-SEL-03** Given selection of `sentry-start` (detection 10 s into a 2-segment event) from the timeline, then
  the slider shows 0 and media starts at 0; focus lands on the cluster trigger containing `sentry-start`.
- [ ] **AT-SEL-04** When `過去の記録` is pressed, then the older event is selected and starts at its own initial
  position (65 for `sentry-preroll`); when `新しい記録` is pressed, then the newer one starts at 0.
- [ ] **AT-SEL-05** Given fixture *full*, when the Sentry row `2026-08-20 20:35:00` is chosen, then playback starts
  and the position reaches ≥ 15; `新しい記録` then starts the Recent event below 1 s; `過去の記録` returns to ≥ 15; the
  Recent marker in `20:35–20:45・3件を開く` starts below 1 s (`interactions.spec.ts`).
- [ ] **AT-SEL-06** (contract) Given `pause()` before any camera is ready, then no play call happens when cameras
  become ready.
- [ ] **AT-SEL-07** Given grid mode and rate 1.5×, when another event is selected, then grid mode is kept, rate is
  `1×`, no camera is expanded and `front` is active (`Theater.test.tsx`).
- [ ] **AT-SEL-08** Given fixture *history*, when `前の記録日 2026-08-19` is pressed, then the date field is
  `2026-08-19`, the row `2026-08-19 19:30:00` is selected and playing; when `次の記録日 2026-08-20` is pressed, the
  date field is `2026-08-20` and the row `2026-08-20 20:45:00` (latest of that day) is selected
  (`timeline-history.spec.ts`, `App.selection.test.tsx`).

### 7.5 Theater

- [ ] **AT-THR-01** Given a Saved event with six cameras, then the context bar shows `2026-08-21 12:00:00`, `保存済み`,
  `6/6 カメラ`, `1分`, and contains the `合成動画を書き出す` and `車両配置` controls; the inspector sits right of the
  stage (`Theater.test.tsx`, `layout.spec.ts`).
- [ ] **AT-THR-02** Given six cameras loaded, when switching `6分割` → `車両配置` → `6分割`, then exactly six media
  acquisitions happened in total and none were released; the six players are the same instances
  (`Theater.test.tsx`, `rich-layout.spec.ts`).
- [ ] **AT-THR-03** Tiles are in canonical order with names `{label}カメラを拡大`; activating `左ピラー` expands it
  (pressed) and activating again collapses it; while expanded the view switch is disabled and all six players stay
  alive; `Escape` collapses and the previous mode (`6分割`) is still selected.
- [ ] **AT-THR-04** Given a window narrower than 1280 pt, then the view switch and vehicle image are hidden and the
  stage is the 3 × 2 grid.
- [ ] **AT-THR-05** Given fixture *missing*, then the Sentry row shows `5/6 カメラ` and `右リピーターなし`; after
  selection the right-repeater tile shows `映像がありません` and the context bar `5/6 カメラ` (`error-states.spec.ts`).
- [ ] **AT-THR-06** Given a clip whose file read fails, then its tile shows `映像を読み込めません`; the other five
  (missing) show `映像がありません`; with `event.mp4` present, the representative section appears only after all six
  are terminal, the view switch disappears, and the section reads `代表映像（単独再生）` /
  `6カメラ同期映像ではありません。` (`Theater.fallback.test.tsx`).
- [ ] **AT-THR-07** Given fixture *corrupt*, then all six tiles end `corrupt` or `unsupported`, the representative
  section is visible, tiles are above it, it is above the transport, the transport and rate control are inside a
  1280 × 1000 window, and no tile label overlaps its status text (`error-states.spec.ts`).
- [ ] **AT-THR-08** Given `event.mp4` cannot be read, then `代表映像を読み込めません。` and no representative player.
- [ ] **AT-THR-09** Given the theater closes while a file is still being opened, then no media handle is created
  afterwards (`Theater.fallback.test.tsx`).
- [ ] **AT-THR-10** Every camera video is aspect-fit (no cropping) and has non-zero dimensions once ready
  (`layout.spec.ts`).
- [ ] **AT-THR-11** Given an event change, then the new session acquires six handles and the old six are released.
- [ ] **AT-THR-12** In spatial mode at ≥ 1280 pt, `front` is above the vehicle image, `back` below, pillars left and
  right of it, repeaters at the bottom corners.

### 7.6 Playback

- [ ] **AT-PB-01** (contract) Given two ready cameras, when `seek(45)`, `skip(10)`, `setRate(1.5)`, `pause()`, then
  elapsed 55, both media times 55, both rates 1.5, both paused.
- [ ] **AT-PB-02** (contract) Given a pending play, when seeking within the segment, skipping or changing rate, then
  after the play resolves the machine is playing and the new time/rate is kept.
- [ ] **AT-PB-03** (contract) Given one camera rejecting and one accepting play, then playing; given all rejecting,
  then paused with `REJECTED`, and a later `loading` → `ready` does not call play.
- [ ] **AT-PB-04** (contract) Only the source camera's time updates elapsed; drift 0.25 s is not corrected, 0.26 s is;
  source time 60 enters segment 1 at offset 0.
- [ ] **AT-PB-05** (contract) Given the preferred (expanded) camera becomes unavailable, then the clock moves to the
  next ready camera; its final `ended` leaves elapsed at the duration and paused.
- [ ] **AT-PB-06** (contract) Given the expanded camera changes to `back`, then `front` updates are ignored and `back`
  drives elapsed.
- [ ] **AT-PB-07** (contract) A repeated `ready` neither rewinds (1.25 s stays 1.25 s) nor replays.
- [ ] **AT-PB-08** (contract) After `ended` into segment 2, cameras autoplay when ready; if all reject → paused with
  `REJECTED`; if one succeeds → playing; if all become terminal while intended → paused with `REJECTED`.
- [ ] **AT-PB-09** Given `hasPrevious = false`, then `新しい記録` is disabled; `過去の記録` triggers the older event; the
  slider max is 60 for one segment; the rate popup defaults to `1×` and accepts `1.5×`; the detection marker sits at
  50 % (Saved, detection 30 s) and at 66.67 % (2 segments, 80 s); it is absent for out-of-range detection
  (`Theater.shortcuts.test.tsx`).
- [ ] **AT-PB-10** Given playback ended at the event end, when `再生` is pressed, then the last segment plays again from
  its start and the position increases (`interactions.spec.ts`; §8 A-36).
- [ ] **AT-PB-11** Given all cameras still loading, when `再生` is pressed, then the message
  `再生できるカメラ映像がありません。` / `読み込み後に再試行してください。` is shown on two lines and playback starts by
  itself once a camera is ready.

### 7.7 Keyboard

- [ ] **AT-KEY-01** Given focus on no control, `6` expands the right repeater (`右リピーターカメラをグリッドへ戻す`),
  `Escape` collapses it, `Space` toggles play/pause, `→` moves the slider to 10 (`Theater.shortcuts.test.tsx`,
  `interactions.spec.ts`).
- [ ] **AT-KEY-02** Given focus on a camera tile button, `→` and `6` do nothing to playback/expansion, but `Escape`
  collapses an expanded camera; given focus on the play button, `→` does not skip.
- [ ] **AT-KEY-03** (Mac) `⌘1`…`⌘6`, `⌘←`, `⌘→` are not consumed by the theater; with the export dialog open, `Space`
  and digits do not reach the theater.

### 7.8 Inspector

- [ ] **AT-INS-01** Given Sentry metadata with city, street, lat, lon, reason and detection `20:45:15`, then the
  inspector shows `場所（Teslaの推定値）` `Sample city Sample street`, `推定座標` `35.0000, 139.0000`, `検知時刻`,
  `再生開始`, `連結再生時間`, `カテゴリ` `セントリー`, `録画開始`; `技術情報` is collapsed and hides the reason until
  expanded (`EventInspector.test.tsx`, `interactions.spec.ts`).
- [ ] **AT-INS-02** Given only `street` and `est_lat`, then `場所（Teslaの推定値）` `Sample street` and no `推定座標`.
- [ ] **AT-INS-03** Given a Saved event with an in-range detection, then `検知時刻` is shown and `再生開始` is not.
- [ ] **AT-INS-04** Given fixture *full*, then `2026-08-20T20:35:30` and
  `SYNTHETIC_PRIVATE_CITY SYNTHETIC_PRIVATE_STREET` / `SYNTHETIC_PRIVATE_LAT, SYNTHETIC_PRIVATE_LON` are displayed and
  never leave the process (§7.11).

### 7.9 Day timeline

- [ ] **AT-TL-01** Given three events at the same second, then opening `12:00–12:00・3件を開く` lists `sentry-event`,
  `saved-event`, `recent-event` in that order with the selected one pressed; choosing `保存済み` selects `saved-event`
  (`DayTimeline.test.tsx`).
- [ ] **AT-TL-02** Given that selection, the (web ≤ 1024 px) neighbour summary would read `セントリー` / `保存済み` /
  `最近の走行` / `3件` — contract only on the Mac (`timelineLabels`).
- [ ] **AT-TL-03** (contract) Cluster threshold at 640: 5 939 s apart → one cluster, 5 940 s → two.
- [ ] **AT-TL-04** Given close events at `20:35`, `20:40`, `20:45` with `20:35` selected, when
  `20:35–20:45・3件を開く` is activated, then the trigger reads `20:35–20:45・3件を閉じる` (expanded) and the member row
  appears; when `2026-08-24 20:40:00 保存済み` is chosen, then that event is selected, its new session shows the
  cluster collapsed again, and keyboard focus is on the cluster trigger (`DayTimeline.test.tsx`,
  `App.selection.test.tsx`, `timeline-history.spec.ts`).
- [ ] **AT-TL-05** `←`/`→` move focus between markers in order; `Home`/`End` jump to the first/last marker.
- [ ] **AT-TL-06** Given the selected event is on the latest recorded day `2026-08-24` and `2026-08-23` holds
  `previous-morning` (08:00) and `previous-evening` (20:00), then `次の記録日` is disabled with the plain label and
  `前の記録日 2026-08-23` is enabled; activating it filters to `2026-08-23` and selects `previous-evening`
  (`DayTimeline.test.tsx`).
- [ ] **AT-TL-07** (contract) A filter date with no events yields `この日のイベントはありません。` (component level).
- [ ] **AT-TL-08** Given a category filter `saved`, then the timeline shows only Saved markers of the selected day;
  given a date filter, the timeline still shows all (category/query-matching) events of the selected event's day.
- [ ] **AT-TL-09** Given fixture *history* at 1280 × 1000, when the cluster is opened, then the deck grows to 268 pt
  and every timeline control stays inside the timeline and the window (`rich-layout.spec.ts`,
  `timeline-history.spec.ts`).

### 7.10 Composite export

- [ ] **AT-EXP-01** Given a six-camera event, when `合成動画を書き出す` is activated, then a modal titled
  `合成動画を書き出す` shows `1280 × 720`, `形式` `MP4`, and `対応ブラウザでは高速に書き出します。…`
  (`CompositeVideoExport.test.tsx`).
- [ ] **AT-EXP-02** Given fixture *full* and the Recent event, when `書き出しを開始` is pressed, then `書き出し中` with a
  progress bar appears, then `書き出し完了 · MP4 · 1280 × 720`; `MP4を保存` offers
  `TeslaCam-2026-08-20_20-45-00-vehicle.mp4`; the saved file is > 1 000 bytes, decodes as 1280 × 720 with duration > 0,
  and pixel (410, 100) at 0.5 s has every channel ≤ 8 (`composite-export.spec.ts`).
- [ ] **AT-EXP-03** Given fixture *export*, then the output duration is > 9.5 s and the export completes in less than
  0.9 × that duration.
- [ ] **AT-EXP-04** Given the dialog is open, when `Escape` is pressed, then the dialog closes and focus returns to the
  trigger.
- [ ] **AT-EXP-05** Given an export in progress, when `中止` is pressed, then `書き出しを中止しました。` is shown and no
  file is offered; selecting another event during export aborts it silently.
- [ ] **AT-EXP-06** Given an event where no camera exists in every segment, then the trigger is disabled and
  `6カメラ映像がないため書き出せません` is shown.
- [ ] **AT-EXP-07** (contract) Layout, contain-fit, vehicle `(594, 260, 92, 184)`, label `リア・記録なし` at (416, 517),
  six scrims of height 24 narrower than 190 px for these labels, label font from the tokens.
- [ ] **AT-EXP-08** (contract) A camera-less middle segment is skipped; each segment's files are released before the
  next segment is opened.
- [ ] **AT-EXP-09** Given cancellation before export starts touching media, then no clip is opened.
- [ ] **AT-EXP-10** Given an event without decodable media, then the error `再生可能な動画がありません。` with `もう一度`.
- [ ] **AT-EXP-11** (contract) Progress values follow §3.6 and the progress clock shows `00:30 / 01:00` at 0.5 of a
  one-segment event.

### 7.11 Privacy and layout

- [ ] **AT-PRIV-01** The Mac app opens no network connection during scan, playback or export (no network-client
  entitlement in the sandbox; e2e equivalents assert no HTTP request carries clip names, timestamps, `ftyp` bytes or
  location values). No telemetry, crash reporting SDK, maps or geocoding.
- [ ] **AT-PRIV-02** The selected folder is not persisted (no security-scoped bookmark, no library cache, no recent
  documents entry); relaunch shows the welcome screen. Logs never contain file names, timestamps, addresses or
  coordinates (web design spec "セキュリティとプライバシー").
- [ ] **AT-LAY-01** At 1280 × 1000 with the Sentry event selected and playing, navigator, stage, inspector and deck lie
  inside the window; no page-level scrolling exists (`rich-layout.spec.ts`).
- [ ] **AT-LAY-02** The header is ≤ 64 pt tall (`layout.spec.ts`).
- [ ] **AT-LAY-03** Every visible button is at least 44 × 44 pt (`interactions.spec.ts`).
- [ ] **AT-LAY-04** Light surfaces have luminance > 0.75, stage and tiles < 0.05; interactive boundaries reach 3:1
  against their fill (`layout.spec.ts`).
- [ ] **AT-LAY-05** No runtime errors are logged during any of the flows above.

---

## 8. Ambiguities, defects and decisions

Each item states what the web does (normative unless a Mac decision is given) and what the Mac does.

| ID | Topic | Web behavior | Mac rule |
|---|---|---|---|
| A-1 | Cancelling a re-pick | `scan-started` discards the loaded library before the picker opens; cancel → welcome | Same end state (welcome). Owner may decide to keep the library on cancel; if so change both apps |
| A-2 | Scanning view during the picker | Visible while the picker is open | MAY appear only after the panel returns |
| A-3 | `NO_PLAYABLE` vs `REJECTED` | Pressing play with all cameras terminal shows `REJECTED` (the later refresh overrides `NO_PLAYABLE` in the same batch); `NO_PLAYABLE` only shows while cameras are loading | Same |
| A-4 | Detection for non-Sentry events | Marker and `検知時刻` appear for any category with an in-range detection; pre-roll and `再生開始` are Sentry-only | Same |
| A-5 | Pre-roll copy when detection < 15 s | `検知の15秒前から開始` and `（検知の15秒前）` are shown although playback starts at 0 | Same (copy is verbatim) |
| A-6 | Cluster width | Clusters are computed for a fixed 640 px track, not the measured rail width | Same constant 640 |
| A-7 | Export availability | Requires a camera present in **every** segment (`available > 0`); events whose cameras vary by segment cannot be exported though they have media | Same |
| A-8 | Export clock | `{MM:SS} / {MM:SS}` uses the nominal 60 s/segment duration while the output uses real clip durations | Same |
| A-9 | Short clips in playback | When the source clip ends before 60 s, playback jumps to the next nominal window | Same |
| A-10 | Desktop bands | CSS breakpoints are viewport widths 1025 and 1280; DESIGN.md phrases spatial availability as "stage ≥ 720 px" | Use window content width 1280 / 1025; minimum window 1025 × 760, default 1280 × 1000 (proposal) |
| A-11 | Expanded camera in spatial mode | CSS keeps the spatial grid areas while expanded, so the result is ill-defined; tests always expand in grid mode | Expanded = single tile fills the stage, vehicle hidden, in both modes |
| A-12 | Font stack | `tokens.css` lists `Inter` first (never bundled) while DESIGN.md mandates the system stack | System font (SF Pro + Hiragino Sans) |
| A-13 | `--motion-fast` | Documented (160 ms) but not defined; the web has no transitions at all | No transitions required; spinners only |
| A-14 | Browser-worded copy | `MacのChromeまたはEdgeに対応しています。`, `ブラウザの再生許可を確認して再試行できます。`, `対応ブラウザでは高速に書き出します。完了まではタブを閉じないでください。` | Guidance line omitted; the other two verbatim until the owner approves Mac wording in the catalog |
| A-15 | Timeline input | Code: category and query filters apply, the date filter does not (`timelineEvents`); the 2026-08-24 spec said "same `matchingEvents`" | Follow the code |
| A-16 | Location display | The 2026-08-21/24 specs said "do not display location"; commit `1ff2a5c` displays it | Follow the code (display, never transmit) |
| A-17 | Enumeration order | Engine-defined; affects ties and duplicate winners | Sort entries by name byte-wise |
| A-18 | Symbolic links | Undefined | Counted, not followed, not collected |
| A-19 | Clips directly in `SavedClips/` / `SentryClips/` | Each file becomes its own event keyed by its file name | Same |
| A-20 | Duplicate `(timestamp, camera)` | Last classified file wins | Same (with sorted enumeration) |
| A-21 | Shortcut modifiers / dialog | Modifiers are not checked; window shortcuts also fire under the modal dialog | Ignore Command/Control/Option combinations; suspend theater shortcuts while the dialog is open |
| A-22 | Representative mode timing | Tile statuses carry across segments, so representative mode can switch on in a later segment; digit shortcuts still expand tiles in that mode | Same |
| A-23 | Warning summary count | `{n}件` counts warning kinds, not files | Same |
| A-24 | String length unit | Metadata limit counts UTF-16 code units | Same (count UTF-16 units in Rust) |
| A-25 | Detection precision | Milliseconds are parsed but ignored in offsets | Same |
| A-26 | Row typography | The row date is unstyled body text (16) and the time is 14 pt bold, although DESIGN.md calls the time the strong element | Same as code |
| A-27 | Display weight | h1 renders at 600; DESIGN.md says 560 | 600 |
| A-28 | `検知時刻` value | Raw metadata string (`T`, fraction and `Z` kept) | Same |
| A-29 | Date filter matching | List: raw prefix of the event timestamp; timeline: parsed date | Same |
| A-30 | Day navigation target | Last marker of the day (ties: last by category order, i.e. Recent) | Same |
| A-31 | Placeholder contrast | Dark text on `--color-theater` (≈ 1.2:1) | Use theater text tokens (deliberate fix; web should follow) |
| A-32 | Undecodable `thumb.png` | Image element with a broken source (browser-dependent rendering) | Show nothing |
| A-33 | Export on event change | Aborted silently, no notice | Same |
| A-34 | Superseded copy in older specs | `再走査`, local-processing badge, `この種類のイベントはありません`, `このカメラ映像はありません`, charcoal theme | Not implemented in code; do not implement |
| A-35 | `event.json` `camera` | Parsed and stored, never displayed | Same |
| A-36 | Play after the end | `play()` restarts the current (last) segment's media from 0 | Same (seek ended players to 0 before playing) |
| A-37 | Segment change while playing | Players pause and the UI shows `再生` until the next segment's cameras are ready, then autoplay resumes | Same; a brief `再生` state is acceptable |

---

## Appendix A. Web → Mac platform mapping (guidance)

| Web primitive | Mac equivalent (non-normative) |
|---|---|
| `showDirectoryPicker({mode: 'read'})` | Open panel, directories only, single selection; sandbox read access to the chosen folder for the session only |
| `FileSystemFileHandle.getFile()` / `File.size` | `stat` during the walk; open lazily when playing/exporting |
| `ObjectUrlRegistry` (blob URLs) | Ref-counted media asset/handle cache per file per theater session |
| `<video muted playsInline preload=metadata>` | One muted player per camera per session, reused across layout changes |
| `loadstart`/src change, `canplay`, `error`, `timeupdate`, `ended` | item replaced, ready to play (first frame available), failed, periodic time observer on the source player (≤ 250 ms period), did-play-to-end |
| `MediaError` codes | AVFoundation error categories (§2.10.4) |
| `requestAnimationFrame` focus restore | Focus the target element after the next frame |
| `matchMedia` breakpoints | Window content width observer |
| `<dialog>.showModal()` | In-window modal sheet/overlay with focus trap; `Escape` = cancel |
| WebCodecs + mediabunny (`CanvasSource`, `VideoSampleSink`) | AVAssetReader (decode) → CoreGraphics/CoreImage compositing into 1280 × 720 pixel buffers → AVAssetWriter (H.264, 30 fps) |
| Canvas `measureText`/`fillText` | CoreText with the system font, medium, 13 pt |
| `<a download>` | Save panel with the default file name |
| CSP `connect-src 'self'` | No network entitlement |

## Appendix B. Reference index

| Spec section | Web sources |
|---|---|
| §1.3 | `teslacam/directory-picker.ts`, `teslacam/directory-input.ts`, `react-app/use-directory-scan.ts` |
| §1.4–§1.10 | `teslacam/filename.ts`, `teslacam/build-library.ts`, `teslacam/scan-limits.ts`, `react-app/components/LibrarySidebar.tsx` |
| §1.11 | `teslacam/event-metadata.ts` |
| §1.12–§1.16 | `react-app/local-timestamp.ts`, `react-app/event-filters.ts`, `react-app/event-summary.ts`, `react-app/event-detection.ts` |
| §1.17–§1.19 | `react-app/playback/*`, `react-app/components/Theater.tsx`, `CameraTile.tsx` |
| §1.20 | `react-app/day-timeline.ts`, `react-app/use-timeline-presentation.ts`, `components/DayTimeline*.tsx`, `TimelineEventButton.tsx` |
| §1.21 | `react-app/app-state.ts`, `react-app/App.tsx` |
| §2 | `react-app/components/*.tsx`, `react-app/styles/*.css`, `react-app/use-theater-shortcuts.ts` |
| §3 | `react-app/export/*`, `public/vehicle-top.png` |
| §4 | `react-app/styles/tokens.css`, `DESIGN.md` |
| §7 | `src/**/*.test.ts(x)`, `tests/e2e/*.spec.ts`, `tests/e2e/support/viewer-fixtures.ts` |

