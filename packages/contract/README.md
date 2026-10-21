# Shared contract

Data shared by `apps/web` and `apps/mac`. Nothing in here is code that either app imports at runtime,
except the two resource files the Mac app embeds at build time.

| Path | What it is | Consumers |
|---|---|---|
| `design-tokens.json` | Design tokens of `DESIGN.md` (colours, type, radii, sizes, stage geometry, contrast pairs) | web `styles/tokens.css` (verified), Mac `teslacam_core::theme` (`include_str!`) |
| `composite-layout.json` | The 1280 × 720 export frame | web exporter, Mac `teslacam_core::export_plan` (`include_str!`) |
| `fixtures/*.json` | Golden input → expected output cases for every pure domain rule | web `apps/web/src/contract/*.test.ts`, Mac `teslacam-core` unit tests |

## Golden fixtures

The web app is the reference implementation. Every fixture file is produced by a vitest file in
`apps/web/src/contract/` that builds deterministic inputs (hand-written cases plus the systematic edge
cases of `docs/specs/2026-10-20-mac-native-parity.md` §1 and §7) and computes the expected values by calling
the real web functions.

```bash
just contract-generate          # or: cd apps/web && npm run contract:generate  (CONTRACT_WRITE=1)
cd apps/web && npm run contract:check   # also part of npm test: committed files must match
cd apps/mac && cargo test -p teslacam-core   # the Mac implementation against the same files
```

- **Regeneration policy.** CI only verifies; it never regenerates. A behaviour change touches the web
  function, the regenerated fixture and the Mac implementation in one change. Review the fixture diff by
  hand: it is the behaviour diff.
- **No silent skips.** The Mac suite fails on a fixture file, a `fn` or a case it does not handle, and a
  test checks that every file in `fixtures/` is consumed.
- **Adding a case.** Add it to the matching `apps/web/src/contract/<file>.test.ts`, run
  `npm run contract:generate`, then make `cargo test -p teslacam-core` pass.

### File format

The envelope is a simplified form of the spec's §6.2 (no JSON Schema files; the vitest builders and the
Rust adapters are typed instead):

```json
{
  "contract": "teslacam-viewer",
  "schemaVersion": 1,
  "module": "filename",
  "source": ["apps/web/src/teslacam/filename.ts"],
  "cases": [
    { "name": "camera front", "fn": "parseClipFileName", "platforms": ["mac"], "note": "…",
      "input": { "name": "2026-08-21_14-30-45-front.mp4" },
      "expected": { "camera": "front", "timestamp": "2026-08-21_14-30-45" } }
  ]
}
```

- `name` is unique within a file. `platforms` (optional, default both) marks Mac-only decisions;
  `note` is free text.
- Text is written with stable key order (envelope and case keys first, every other key sorted),
  2-space indent and a trailing newline, so the check can compare text exactly.
- Numbers compare with an absolute tolerance of `1e-9` (`floatTolerance` overrides it). Non-finite
  number inputs are the strings `"NaN"`, `"Infinity"` and `"-Infinity"`.
- A string input may be a generated string `{"$repeat": "x", "count": 2049}` (= `"x"` repeated 2049
  times); `expected` values are always written out.
- `null` stands for both `null` and `undefined` results.

### Compact inputs

- **Event** (`EventFixture`): `{id, category, timestamp, segments: [{timestamp, cameras: [cameraId…]}],
  metadata?, hasRepresentative?, hasThumbnail?}`. Each listed camera gets a placeholder file.
- **Library input file**: `{relativePath, name?, size?, text?}`; `name` defaults to the last `/`
  component, `size` to the UTF-8 byte length of `text`, else 14 (the default contents
  `synthetic-file`). `generate: {count, pathPattern, size?}` appends generated files (`{i}` = index).
  Expected events list camera files by `relativePath`.
- **Directory tree** (`directory-walk.json`): nodes `{kind: "file", name, size}`,
  `{kind: "directory", name, entries?, generate?: {count, namePattern, size?}}` and
  `{kind: "symlink", name}`, in enumeration order. The Mac walker sorts entries by name; the fixture
  adapter feeds them in the given order, and name sorting is tested natively.
- **Metadata read** (`readEventMetadata`): exactly one of `text` (UTF-8 encoded by the adapter) or
  `base64` (raw bytes), plus `declaredSize` (default: the byte length).

### Files

| File | Functions (`fn`) | Web source |
|---|---|---|
| `clip-filename.json` | `parseClipFileName` | `teslacam/filename.ts` |
| `local-timestamp.json` | `parseFilenameLocalTimestamp`, `parseMetadataLocalTimestamp`, `formatFilenameTimestampAtOffset` | `react-app/local-timestamp.ts` |
| `event-metadata.json` | `parseEventMetadata`, `readEventMetadata` | `teslacam/event-metadata.ts` |
| `directory-walk.json` | `walkDirectory` | `teslacam/directory-picker.ts` |
| `library-build.json` | `buildTeslaCamLibrary` | `teslacam/build-library.ts` |
| `event-format-filters.json` | `eventDate`, `eventClock`, `eventTime`, `categoryLabel`, `eventMatchesFilters`, `orderedMatchingEvents` | `react-app/event-filters.ts` |
| `event-summary.json` | `cameraAvailability`, `cameraAvailabilityLabel`, `missingCameraLabel`, `eventDuration`, `eventDurationLabel`, `segmentCountLabel` | `react-app/event-summary.ts` |
| `event-detection.json` | `detectionSeconds`, `sentryInitialElapsedSeconds`, `sentryPlaybackStartLabel` | `react-app/event-detection.ts` |
| `playback-timeline.json` | `eventDuration`, `locatePlaybackTime` | `react-app/playback/timeline.ts` |
| `day-timeline.json` | `selectTimelineDate`, `buildDayTimelineMarkers`, `buildTimelineDays`, `adjacentTimelineDays`, `buildDayTimelineClusters` | `react-app/day-timeline.ts` |
| `copy-format.json` | `cameraLabel`, `categoryLabel`, `formatDuration`, `cameraAvailabilityLabel`, `missingCameraLabel`, `segmentCountLabel`, `eventTime`, and the garage-floor copy: `shortCategoryLabel`, `startClock`, `cameraCountText`, `missingCamerasText`, `detectionFact`, `detectionSentence`, `playbackStartText`, `locationText`, `coordinatesText`, `dayTitle`, `transportTimeLabel`, `scrubberValueText`, `scanProgressCopy`, `warningText` | `react-app/event-summary.ts`, `react-app/event-filters.ts`, `react-app/presentation/event-copy.ts` |

Not covered by fixtures, because they are UI state machines or have no pure web function yet: app state,
playback machine, export state, theater shortcuts, export layout and plan, rail layout. The Mac tests those
natively in `teslacam-core`. `playbackStartText` cases use a regular 60 s cadence, where the web's nominal
axis and the Mac's wall-clock axis agree.

### Mac-only rules pinned by fixtures

- **Symbolic links** are counted against the entry limit but neither followed nor collected
  (`directory-walk.json`, cases with `"platforms": ["mac"]`). The web reference has no symbolic links, so
  its adapter walks each one as a hidden placeholder file, which has exactly that effect.

### Known divergences

- A lone UTF-16 surrogate written as a `\ud800` escape inside `event.json` is valid for `JSON.parse` but
  not for `serde_json`; the Mac treats such a file as unreadable (`invalid-metadata-file`). No fixture
  pins it.

## Mac event timeline (wall-clock)

The web plays every segment in a nominal 60 s window (`playback-timeline.json`). The Mac app places
segments on the wall clock instead (`teslacam_core::event_timeline`):

- Segment *i* starts at `ordinalSeconds(timestamp_i) − ordinalSeconds(timestamp_0)` seconds.
- Each camera's file sits at its segment's start and is truncated where the next segment starts. A
  missing file, an undecodable file or the end of a short clip leaves a gap for that camera.
- A tail shorter than 1 s between the end of a clip and the end of its window (the next segment's
  start, or the event end) holds the clip's last frame instead of becoming a gap, so a 59.97 s clip
  followed by a segment at 60 s never flashes a missing window. `teslacam-core` (`event_timeline`) and
  `teslacam-media` (`timeline`) apply the same rule, and the composite export draws no `記録なし` there.
- The event duration is the start of the last segment plus the longest clip of that segment (60 s when no
  clip length is known yet).
- Playback seconds map to `(segment index, offset)`: the segment is the last one that starts at or before
  the time, boundaries belong to the next segment and the end belongs to the last one. The wall-clock label
  is the segment timestamp plus the whole seconds of the offset.
- With a regular 60 s cadence and 60 s clips this is identical to the web's 60 s math (tested against
  `playback-timeline.json`).

## Fixed on the Mac (and to be fixed on the web)

The spec header lists behaviour bugs that are fixed rather than copied. The fixtures keep the web's
current behaviour; the Mac app layers the fix on top and tests it natively:

| Topic | Web today (fixture) | Mac |
|---|---|---|
| Detection marker on non-Sentry events | `detectionSeconds` is non-null for any category | marker and `検知` only for Sentry (`detection::sentry_detection`) |
| Pre-roll label when the detection is earlier than 15 s | `sentryPlaybackStartLabel` returns the first segment's start | no pre-roll (`SentryDetection::pre_roll_start` is `None`) |
| Cluster width | fixed 640 px | measured rail width (the function already takes the width; the fixture covers several) |
| "No playable camera" message | overwritten by the rejected-playback message | kept (`playback::PlaybackMachine`) |
| Cancelling a re-pick | discards the loaded library | keeps it (`app_state::AppModel`) |

## Other Mac decisions (not pinned by fixtures)

Behaviour the Mac implements natively in `teslacam-core`; the web may follow when it adopts the garage
floor:

- A freshly loaded library selects its newest event (the stage fills with video, `DESIGN.md` §7).
- Choosing a day (date picker, `前の記録日`, `次の記録日`) filters to it and selects its newest event.
- The representative view opens automatically only when every camera is terminal and at least one has a
  file fault; a gap where every camera is merely missing keeps the camera view.
- The composite export follows the wall-clock timeline (gaps are drawn as `記録なし`), and
  `ExportPlan::frame_count` is `ceil(duration × 30)`.
- Browser-worded copy is not rendered: the rejected-playback message is `映像を再生できませんでした。`
  alone and the export dialog has no "keep the tab open" notice.
