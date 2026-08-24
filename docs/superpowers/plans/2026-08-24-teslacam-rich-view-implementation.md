# TeslaCam Viewer Rich View Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the approved rich TeslaCam investigation workspace, including Sentry playback from 15 seconds before a valid detection, a non-wrapping event navigator, spatial six-camera view, factual event inspector, and responsive day timeline.

**Architecture:** Keep `AppState` as the selection and filter source of truth, keep one event-keyed playback session, and preserve the existing six `CameraTile` elements and `ObjectUrlRegistry` lifecycle. Add pure local-time and day-timeline models, compose them through `ReadyWorkspace`, and use CSS placement to switch the same camera DOM between spatial and 3×2 grid modes.

**Tech Stack:** TypeScript 6, React 19, Vite 8, Hono 4, Vitest, Testing Library, Playwright, Cloudflare Workers, Wrangler

**Spec:** `docs/superpowers/specs/2026-08-24-teslacam-rich-view-design.md`

## Global Constraints

- Keep the default theme light and scope dark tokens to the evidence stage and day timeline.
- Do not add comments to implementation files.
- Do not use `any`, `as any`, `@ts-ignore`, or `@ts-expect-error`.
- Avoid new `useEffect` calls; retain the existing media and Blob URL lifecycle effects where they are required.
- Do not add external fetches, analytics, telemetry, maps, authentication, or cloud media processing.
- Never render `city`, `street`, `est_lat`, or `est_lon`.
- Keep `object-fit: contain` on every TeslaCam video.
- Keep the same six camera video elements mounted across spatial, grid, and expanded views.
- A repeated ready notification for the same media lifecycle must not reset `currentTime`.
- Sentry selection starts paused at `max(0, detectionSeconds - 15)`; SavedClips and RecentClips start paused at zero.
- Validate 1280×1000, 768×1024, and 375×844 without page-level horizontal overflow.
- Use the existing Lucide icon dependency and the existing token system; do not add a UI framework.
- Generate the neutral vehicle asset with ImageGen; do not draw it with CSS, inline SVG, emoji, or text symbols.

---

### Task 1: Local timestamp and Sentry detection model

**Files:**

- Create: `src/react-app/local-timestamp.ts`
- Create: `src/react-app/local-timestamp.test.ts`
- Modify: `src/react-app/event-detection.ts`
- Create: `src/react-app/event-detection.test.ts`
- Modify: `src/react-app/components/Theater.test.tsx`

**Interfaces:**

- Consumes: `TeslaCamEvent`, ordered `TeslaCamEvent.segments`, and metadata timestamp strings.
- Produces: `parseFilenameLocalTimestamp(value: string): LocalTimestamp | null`, `parseMetadataLocalTimestamp(value: string): LocalTimestamp | null`, `formatFilenameTimestampAtOffset(value: string, offsetSeconds: number): string | null`, `detectionSeconds(event: TeslaCamEvent): number | null`, `sentryInitialElapsedSeconds(event: TeslaCamEvent): number`, and `sentryPlaybackStartLabel(event: TeslaCamEvent): string | null`.

- [ ] **Step 1: Add failing strict local timestamp tests**

```ts
expect(parseFilenameLocalTimestamp("2026-08-24_23-59-59")).toMatchObject({
  dateKey: "2026-08-24",
  timeLabel: "23:59:59",
  secondsOfDay: 86_399,
});
expect(parseFilenameLocalTimestamp("2026-02-29_12-00-00")).toBeNull();
expect(parseFilenameLocalTimestamp("2028-02-29_12-00-00")).not.toBeNull();
expect(
  parseMetadataLocalTimestamp("2026-08-24T12:00:30.250Z")?.millisecond,
).toBe(250);
expect(parseMetadataLocalTimestamp("2026-08-24T12:00:30+09:00")).toBeNull();
expect(parseMetadataLocalTimestamp("2026-08-24_12-00-30")).toBeNull();
expect(formatFilenameTimestampAtOffset("2026-12-31_23-59-45", 30)).toBe(
  "2027-01-01 00:00:15",
);
```

- [ ] **Step 2: Run the local timestamp tests and confirm the missing-module failure**

```bash
npm test -- --run src/react-app/local-timestamp.test.ts
```

Expected: FAIL because `local-timestamp.ts` does not exist.

- [ ] **Step 3: Implement calendar validation and local arithmetic without `Date`**

```ts
export type LocalTimestamp = {
  readonly year: number;
  readonly month: number;
  readonly day: number;
  readonly hour: number;
  readonly minute: number;
  readonly second: number;
  readonly millisecond: number;
  readonly dateKey: string;
  readonly timeLabel: string;
  readonly secondsOfDay: number;
  readonly ordinalSeconds: number;
};

export function parseFilenameLocalTimestamp(
  value: string,
): LocalTimestamp | null;
export function parseMetadataLocalTimestamp(
  value: string,
): LocalTimestamp | null;
export function formatFilenameTimestampAtOffset(
  value: string,
  offsetSeconds: number,
): string | null;
```

Use integer leap-year and month-length helpers. Build `ordinalSeconds` from calendar parts, and treat a trailing metadata `Z` as an accepted Tesla string suffix without applying a timezone conversion.

- [ ] **Step 4: Run the timestamp tests and confirm they pass**

```bash
npm test -- --run src/react-app/local-timestamp.test.ts
```

Expected: PASS.

- [ ] **Step 5: Add failing segment-aware detection tests**

```ts
expect(
  detectionSeconds(
    sentryEvent({
      segments: ["2026-08-24_12-00-00", "2026-08-24_12-01-00"],
      detection: "2026-08-24T12:01:20",
    }),
  ),
).toBe(80);
expect(
  sentryInitialElapsedSeconds(
    sentryEvent({
      segments: ["2026-08-24_12-00-00", "2026-08-24_12-01-00"],
      detection: "2026-08-24T12:01:20",
    }),
  ),
).toBe(65);
expect(
  detectionSeconds(
    sentryEvent({
      segments: ["2026-08-24_12-00-00", "2026-08-24_12-01-11"],
      detection: "2026-08-24T12:01:05",
    }),
  ),
).toBeNull();
expect(
  detectionSeconds(
    sentryEvent({
      segments: ["2026-08-24_12-00-00", "2026-08-24_12-00-50"],
      detection: "2026-08-24T12:00:55",
    }),
  ),
).toBe(55);
expect(sentryInitialElapsedSeconds(savedEventWithDetection)).toBe(0);
```

Cover the first 10 seconds, exact 60-second boundary, final segment endpoint, overlap selecting the smallest segment index, midnight, malformed metadata, timezone offsets, and out-of-range metadata.

- [ ] **Step 6: Run the detection tests and confirm the old signature or expectations fail**

```bash
npm test -- --run src/react-app/event-detection.test.ts src/react-app/components/Theater.test.tsx
```

Expected: FAIL because detection is still event-start based and requires a duration argument.

- [ ] **Step 7: Implement segment-aware detection and pre-roll helpers**

```ts
export function detectionSeconds(event: TeslaCamEvent): number | null {
  const detected = parseMetadataLocalTimestamp(event.metadata?.timestamp ?? "");
  if (detected === null) return null;
  const finalIndex = event.segments.length - 1;
  for (const [index, segment] of event.segments.entries()) {
    const start = parseFilenameLocalTimestamp(segment.timestamp);
    if (start === null) continue;
    const offset = detected.ordinalSeconds - start.ordinalSeconds;
    const inRange =
      index === finalIndex
        ? offset >= 0 && offset <= 60
        : offset >= 0 && offset < 60;
    if (inRange) return index * 60 + offset;
  }
  return null;
}

export function sentryInitialElapsedSeconds(event: TeslaCamEvent): number {
  if (event.category !== "sentry") return 0;
  const detected = detectionSeconds(event);
  return detected === null ? 0 : Math.max(0, detected - 15);
}
```

Derive `sentryPlaybackStartLabel` by locating the initial elapsed position, selecting that segment timestamp, and formatting the segment-relative offset through `formatFilenameTimestampAtOffset`.

- [ ] **Step 8: Run focused tests, typecheck, and lint**

```bash
npm test -- --run src/react-app/local-timestamp.test.ts src/react-app/event-detection.test.ts src/react-app/components/Theater.test.tsx
npm run typecheck
npm run lint
```

Expected: PASS.

- [ ] **Step 9: Commit the pure time model**

```bash
git add src/react-app/local-timestamp.ts src/react-app/local-timestamp.test.ts src/react-app/event-detection.ts src/react-app/event-detection.test.ts src/react-app/components/Theater.test.tsx
git commit -m "セントリー検知時刻の計算を正確にする"
```

---

### Task 2: Paused playback at the Sentry pre-roll position

**Files:**

- Modify: `src/react-app/playback/playback-runtime.ts`
- Modify: `src/react-app/playback/use-synchronized-playback.ts`
- Modify: `src/react-app/playback/use-synchronized-playback.test.tsx`
- Modify: `src/react-app/playback/use-synchronized-playback-transitions.test.tsx`
- Modify: `src/react-app/components/Theater.tsx`
- Modify: `src/react-app/components/Theater.test.tsx`

**Interfaces:**

- Consumes: `sentryInitialElapsedSeconds(event)` from Task 1 and `locatePlaybackTime(event, elapsedSeconds)`.
- Produces: `createPlaybackRuntime(elapsedSeconds?: number, segmentIndex?: number): PlaybackRuntime` and `useSynchronizedPlayback(event: TeslaCamEvent, preferredCamera?: CameraId | null, initialElapsedSeconds?: number): SynchronizedPlayback`.

- [ ] **Step 1: Add failing non-zero initialization tests**

```ts
const hook = renderHook(() =>
  useSynchronizedPlayback(eventWithSegments(2), null, 65),
);
expect(hook.result.current.elapsedSeconds).toBe(65);
expect(hook.result.current.segmentIndex).toBe(1);
expect(hook.result.current.segmentOffsetSeconds).toBe(5);
expect(hook.result.current.playing).toBe(false);
```

Register a video, send `ready`, expect `currentTime === 5`, change `currentTime` to `9`, send `ready` again, and expect it to remain `9`.

- [ ] **Step 2: Run playback tests and confirm zero initialization fails the new assertions**

```bash
npm test -- --run src/react-app/playback/use-synchronized-playback.test.tsx src/react-app/playback/use-synchronized-playback-transitions.test.tsx
```

Expected: FAIL because the hook and runtime always initialize at zero.

- [ ] **Step 3: Initialize React and runtime state synchronously**

```ts
export function createPlaybackRuntime(
  elapsedSeconds = 0,
  segmentIndex = 0,
): PlaybackRuntime;

export function useSynchronizedPlayback(
  event: TeslaCamEvent,
  preferredCamera: CameraId | null = null,
  initialElapsedSeconds = 0,
): SynchronizedPlayback;
```

Compute one clamped initial location with `locatePlaybackTime`. Use lazy `useState` initialization for elapsed seconds and `PlaybackRuntime`. Keep `intended` false and preserve the existing `wasReady` guard.

- [ ] **Step 4: Connect Theater to the pre-roll helper without an effect**

```ts
const detectedSeconds = detectionSeconds(event);
const initialElapsedSeconds = sentryInitialElapsedSeconds(event);
const playback = useSynchronizedPlayback(
  event,
  expandedCamera,
  initialElapsedSeconds,
);
```

Keep `<TheaterSession key={event.id}>` as the event reset boundary at this stage.

- [ ] **Step 5: Add Theater integration assertions**

```ts
expect(screen.getByLabelText("再生位置")).toHaveValue("65");
expect(screen.getByRole("button", { name: "再生" })).toBeVisible();
fireEvent.canPlay(screen.getAllByTestId("camera-video")[0]);
expect(screen.getAllByTestId("camera-video")[0]).toHaveProperty(
  "currentTime",
  5,
);
```

Add list selection and previous/next selection cases that each create a fresh paused Theater at the selected event’s own initial position. Re-selecting the same current event must not remount or seek.

- [ ] **Step 6: Run the focused playback and Theater suite**

```bash
npm test -- --run src/react-app/event-detection.test.ts src/react-app/playback/use-synchronized-playback.test.tsx src/react-app/playback/use-synchronized-playback-transitions.test.tsx src/react-app/components/Theater.test.tsx src/react-app/App.test.tsx
npm run typecheck
npm run lint
```

Expected: PASS.

- [ ] **Step 7: Commit playback initialization**

```bash
git add src/react-app/playback/playback-runtime.ts src/react-app/playback/use-synchronized-playback.ts src/react-app/playback/use-synchronized-playback.test.tsx src/react-app/playback/use-synchronized-playback-transitions.test.tsx src/react-app/components/Theater.tsx src/react-app/components/Theater.test.tsx src/react-app/App.test.tsx
git commit -m "セントリーを検知15秒前から開く"
```

---

### Task 3: Day timeline model and accessible event selection

**Files:**

- Create: `src/react-app/day-timeline.ts`
- Create: `src/react-app/day-timeline.test.ts`
- Create: `src/react-app/components/DayTimeline.tsx`
- Create: `src/react-app/components/DayTimeline.test.tsx`
- Create: `src/react-app/components/PlaybackDeck.tsx`
- Modify: `src/react-app/components/ReadyWorkspace.tsx`
- Modify: `src/react-app/components/Theater.tsx`

**Interfaces:**

- Consumes: `LocalTimestamp`, `EventFilters['dateKey']`, the existing `matchingEvents`, and `onSelectEvent(eventId)`.
- Produces: `DayTimelineMarker`, `selectTimelineDate(events, selectedEvent, filterDateKey)`, `buildDayTimelineMarkers(events, dateKey)`, and `DayTimeline`.

- [ ] **Step 1: Add failing pure timeline tests**

```ts
expect(selectTimelineDate(events, selectedEvent, "2026-08-23")).toBe(
  "2026-08-24",
);
expect(selectTimelineDate(events, undefined, "2026-08-23")).toBe("2026-08-23");
expect(buildDayTimelineMarkers(events, "2026-08-24")[0]?.positionPercent).toBe(
  0,
);
expect(
  buildDayTimelineMarkers(eventsAtEndOfDay, "2026-08-24")[0]?.secondsOfDay,
).toBe(86_399);
expect(
  buildDayTimelineMarkers(sameSecondEvents, "2026-08-24").map(
    (marker) => marker.category,
  ),
).toEqual(["sentry", "saved", "recent"]);
```

Cover empty results, leap day, year boundary, a RecentClips session assigned to its first segment date, filtered inputs, and event ID ordering within one category.

- [ ] **Step 2: Run the pure test and confirm the missing-module failure**

```bash
npm test -- --run src/react-app/day-timeline.test.ts
```

Expected: FAIL because `day-timeline.ts` does not exist.

- [ ] **Step 3: Implement the pure day model**

```ts
export type DayTimelineMarker = {
  readonly eventId: string;
  readonly category: ClipCategory;
  readonly dateKey: string;
  readonly timeLabel: string;
  readonly secondsOfDay: number;
  readonly positionPercent: number;
};

export function selectTimelineDate(
  events: readonly TeslaCamEvent[],
  selectedEvent: TeslaCamEvent | undefined,
  filterDateKey: string | null,
): string | null;

export function buildDayTimelineMarkers(
  events: readonly TeslaCamEvent[],
  dateKey: string | null,
): readonly DayTimelineMarker[];
```

Sort equal seconds by `sentry`, `saved`, `recent`, then `eventId.localeCompare`.

- [ ] **Step 4: Add failing DayTimeline component tests**

```tsx
render(
  <DayTimeline
    events={sameSecondEvents}
    filterDateKey={null}
    selectedEvent={sameSecondEvents[0]}
    selectedEventId={sameSecondEvents[0]?.id ?? null}
    onSelectEvent={onSelectEvent}
  />,
);
expect(screen.getAllByRole("button", { name: /2026-08-24/ })).toHaveLength(3);
await user.click(screen.getByRole("button", { name: /保存済み/ }));
expect(onSelectEvent).toHaveBeenCalledWith("saved-event");
```

Assert the DOM order, `aria-pressed`, empty-day copy, selected/previous/next compact summaries, and ArrowLeft/ArrowRight focus order.

- [ ] **Step 5: Implement DayTimeline without effects**

```ts
type DayTimelineProps = {
  readonly events: readonly TeslaCamEvent[];
  readonly filterDateKey: string | null;
  readonly selectedEvent: TeslaCamEvent | undefined;
  readonly selectedEventId: string | null;
  readonly onSelectEvent: (eventId: string) => void;
};
```

Use one `useRef<Array<HTMLButtonElement | null>>` for arrow-key focus. Render independent marker buttons in stable order, a desktop ruler, a tablet event-time list, and a mobile previous/selected/next summary from the same marker model. Give each marker `data-day-event-id={marker.eventId}`. After dispatching selection, schedule one `requestAnimationFrame` that finds the new button by exact `dataset.dayEventId` and restores focus without a React effect.

- [ ] **Step 6: Compose selected-event controls and the day timeline as PlaybackDeck**

```tsx
type PlaybackDeckProps = {
  readonly controls: ReactNode;
  readonly timeline: ReactNode;
};

export function PlaybackDeck({ controls, timeline }: PlaybackDeckProps) {
  return (
    <div className="playback-deck">
      {controls}
      {timeline}
    </div>
  );
}
```

Render `PlaybackDeck` inside `TheaterSession`, keep `TheaterSession` keyed by `event.id`, and rely on the marker’s post-dispatch exact-ID focus restoration when selection remounts the session.

- [ ] **Step 7: Pass the exact `matchingEvents` collection from ReadyWorkspace**

```tsx
<Theater
  event={selectedEvent}
  timelineEvents={matchingEvents}
  timelineDateKey={state.filters.dateKey}
  selectedEventId={state.selectedEventId}
  onSelectEvent={onSelectEvent}
  {...navigationProps}
/>
```

- [ ] **Step 8: Run timeline, integration, type, and lint checks**

```bash
npm test -- --run src/react-app/day-timeline.test.ts src/react-app/components/DayTimeline.test.tsx src/react-app/components/Theater.test.tsx src/react-app/App.test.tsx
npm run typecheck
npm run lint
```

Expected: PASS.

- [ ] **Step 9: Commit the day timeline**

```bash
git add src/react-app/day-timeline.ts src/react-app/day-timeline.test.ts src/react-app/components/DayTimeline.tsx src/react-app/components/DayTimeline.test.tsx src/react-app/components/PlaybackDeck.tsx src/react-app/components/ReadyWorkspace.tsx src/react-app/components/Theater.tsx src/react-app/components/Theater.test.tsx src/react-app/App.test.tsx
git commit -m "日内イベントタイムラインを追加する"
```

---

### Task 4: Event navigator and factual event inspector

**Files:**

- Create: `src/react-app/components/EventNavigator.tsx`
- Create: `src/react-app/components/EventInspector.tsx`
- Create: `src/react-app/components/EventInspector.test.tsx`
- Modify: `src/react-app/components/LibrarySidebar.tsx`
- Modify: `src/react-app/components/EventList.tsx`
- Modify: `src/react-app/components/EventList.test.tsx`
- Modify: `src/react-app/components/ReadyWorkspace.tsx`
- Modify: `src/react-app/event-filters.ts`
- Modify: `src/react-app/event-summary.ts`

**Interfaces:**

- Consumes: `matchingEvents`, existing filters and counts, Sentry detection helpers, and existing camera summary helpers.
- Produces: `EventNavigator`, record-style event rows, `eventDate(timestamp)`, `eventClock(timestamp)`, and `EventInspector`.

- [ ] **Step 1: Add failing event-row semantics tests**

```ts
expect(
  screen.getByRole("button", { name: /2026-08-20 20:45:00/ }),
).toHaveTextContent("2026-08-20");
expect(
  screen.getByRole("button", { name: /2026-08-20 20:45:00/ }),
).toHaveTextContent("20:45:00");
expect(screen.getByText("セントリー")).toHaveClass("event-category");
expect(
  screen.getByText("セントリー").closest(".event-category"),
).toHaveAttribute("data-atomic", "true");
expect(
  screen.getByRole("button", { name: /2026-08-20 20:45:00/ }),
).toHaveTextContent("検知の15秒前から開始");
```

Add the pre-roll note only for a selected Sentry event with a valid detection timestamp.

- [ ] **Step 2: Add failing inspector privacy and factual-data tests**

```tsx
render(<EventInspector event={eventWithLocationMetadata} />);
expect(screen.getByText("セントリー")).toBeVisible();
expect(screen.getByText("録画開始")).toBeVisible();
expect(screen.getByText("検知時刻")).toBeVisible();
expect(screen.getByText("再生開始")).toBeVisible();
expect(screen.getByText("連結再生時間")).toBeVisible();
expect(screen.getByText("sentry_aware_object_detection")).toBeVisible();
expect(screen.queryByText("Sample city")).not.toBeInTheDocument();
expect(screen.queryByText("Sample street")).not.toBeInTheDocument();
expect(screen.queryByText("35.0000")).not.toBeInTheDocument();
```

- [ ] **Step 3: Run component tests and confirm the new structure is absent**

```bash
npm test -- --run src/react-app/components/EventList.test.tsx src/react-app/components/EventInspector.test.tsx
```

Expected: FAIL.

- [ ] **Step 4: Split date/time and protect category labels**

```ts
export function eventDate(timestamp: string): string {
  return timestamp.slice(0, 10);
}

export function eventClock(timestamp: string): string {
  return timestamp.slice(11).replaceAll("-", ":");
}
```

Render `.event-card__date`, `.event-card__time`, `.event-category[data-atomic="true"]`, and the selected valid-Sentry pre-roll note. Keep `EventThumbnail` as the only event-list thumbnail Blob URL owner.

- [ ] **Step 5: Consolidate sidebar and list behind EventNavigator**

```ts
type EventNavigatorProps = {
  readonly events: readonly TeslaCamEvent[];
  readonly selectedEventId: string | null;
  readonly counts: Readonly<Record<ClipCategory, number>>;
  readonly eventCount: number;
  readonly filters: EventFilters;
  readonly folderName: string;
  readonly warnings: readonly ScanWarning[];
  readonly onCategory: (category: EventFilters["category"]) => void;
  readonly onDate: (dateKey: string | null) => void;
  readonly onQuery: (query: string) => void;
  readonly onSelectEvent: (eventId: string) => void;
};
```

Make `EventNavigator` the single desktop `aside`; change `LibrarySidebar` to a non-landmark section so the navigator does not contain nested asides.

- [ ] **Step 6: Implement the allow-listed EventInspector**

```ts
type EventInspectorProps = {
  readonly event: TeslaCamEvent;
};
```

Render category, recording start, valid detection timestamp, valid playback-start label, `eventDurationLabel`, segment count, camera availability, missing camera directions, and raw `metadata.reason`. Do not read or spread location fields.

- [ ] **Step 7: Run component and app checks**

```bash
npm test -- --run src/react-app/components/EventList.test.tsx src/react-app/components/EventInspector.test.tsx src/react-app/App.test.tsx
npm run typecheck
npm run lint
```

Expected: PASS.

- [ ] **Step 8: Commit the navigator and inspector**

```bash
git add src/react-app/components/EventNavigator.tsx src/react-app/components/EventInspector.tsx src/react-app/components/EventInspector.test.tsx src/react-app/components/LibrarySidebar.tsx src/react-app/components/EventList.tsx src/react-app/components/EventList.test.tsx src/react-app/components/ReadyWorkspace.tsx src/react-app/event-filters.ts src/react-app/event-summary.ts
git commit -m "イベント調査ナビゲーションを再構成する"
```

---

### Task 5: Spatial six-camera stage and persistent view mode

**Files:**

- Create: `public/vehicle-top.png`
- Create: `src/react-app/components/TheaterViewSwitch.tsx`
- Create: `src/react-app/components/CameraStage.tsx`
- Modify: `src/react-app/components/CameraTile.tsx`
- Modify: `src/react-app/components/ReadyWorkspace.tsx`
- Modify: `src/react-app/components/Theater.tsx`
- Modify: `src/react-app/components/Theater.test.tsx`

**Interfaces:**

- Consumes: canonical `cameraIds`, existing `CameraTile` callbacks, `ObjectUrlRegistry`, and event-keyed `TheaterSession`.
- Produces: `TheaterViewMode = 'spatial' | 'grid'`, `TheaterViewSwitch`, and `CameraStage`.

- [ ] **Step 1: Generate the real neutral vehicle asset with ImageGen**

```text
Create a single top-down orthographic image of a modern neutral graphite electric sedan, centered vertically, no logo, no brand marks, no text, no arrows, no people, no environment, soft studio lighting, transparent background, suitable for a compact dark evidence dashboard at approximately 120 by 240 CSS pixels. Keep the silhouette realistic and symmetrical.
```

Save the selected PNG as `public/vehicle-top.png`. Inspect it at the target slot before wiring it into the stage.

- [ ] **Step 2: Add failing view-mode and Blob lifecycle tests**

```ts
expect(screen.getByRole("button", { name: "車両配置" })).toHaveAttribute(
  "aria-pressed",
  "true",
);
await user.click(screen.getByRole("button", { name: "6分割" }));
expect(screen.getAllByTestId("camera-video")).toHaveLength(6);
expect(URL.createObjectURL).toHaveBeenCalledTimes(6);
expect(URL.revokeObjectURL).not.toHaveBeenCalled();
await user.click(screen.getByRole("button", { name: "車両配置" }));
expect(URL.createObjectURL).toHaveBeenCalledTimes(6);
```

Expand a camera, assert the switch is disabled, assert all six videos remain in the DOM, press Escape, and assert the previous mode returns. Rerender with a different event and assert the selected view mode persists while the media session reacquires the new event files.

- [ ] **Step 3: Run Theater tests and confirm view controls are missing**

```bash
npm test -- --run src/react-app/components/Theater.test.tsx
```

Expected: FAIL.

- [ ] **Step 4: Implement the stateless switch and stage wrapper**

```ts
export type TheaterViewMode = "spatial" | "grid";

type TheaterViewSwitchProps = {
  readonly mode: TheaterViewMode;
  readonly disabled: boolean;
  readonly onChange: (mode: TheaterViewMode) => void;
};

type CameraStageProps = {
  readonly mode: TheaterViewMode;
  readonly expanded: boolean;
  readonly children: ReactNode;
};
```

Render the six `CameraTile` children exactly once in canonical order. Add `data-camera={cameraId}` to each tile root. Use `data-view-mode` on `CameraStage`; never branch into separate spatial and grid camera trees.

- [ ] **Step 5: Keep view-mode state in ReadyWorkspace outside the event-keyed session**

```ts
export function ReadyWorkspace(props: ReadyWorkspaceProps) {
  const [viewMode, setViewMode] = useState<TheaterViewMode>('spatial')
  return <Theater {...theaterProps} viewMode={viewMode} onViewMode={setViewMode} />
}
```

Add `viewMode: TheaterViewMode` and `onViewMode: (mode: TheaterViewMode) => void` to `TheaterProps`, and pass them into `TheaterSession`. Hide the switch in representative fallback mode and disable it while a camera is expanded. The neutral vehicle `<img src="/vehicle-top.png" alt="" />` may be conditional; camera children may not be conditional on view mode.

- [ ] **Step 6: Place EventInspector beside the stage**

```tsx
<div className="theater-stage-row">
  <CameraStage mode={viewMode} expanded={expandedCamera !== null}>
    {cameraTiles}
  </CameraStage>
  <EventInspector event={event} />
</div>
```

- [ ] **Step 7: Run focused React, URL registry, type, and lint checks**

```bash
npm test -- --run src/react-app/components/Theater.test.tsx src/react-app/playback/object-url-registry.test.ts src/react-app/playback/use-synchronized-playback-transitions.test.tsx
npm run typecheck
npm run lint
```

Expected: PASS.

- [ ] **Step 8: Commit the spatial stage**

```bash
git add public/vehicle-top.png src/react-app/components/TheaterViewSwitch.tsx src/react-app/components/CameraStage.tsx src/react-app/components/CameraTile.tsx src/react-app/components/ReadyWorkspace.tsx src/react-app/components/Theater.tsx src/react-app/components/Theater.test.tsx
git commit -m "6カメラを車両配置で表示する"
```

---

### Task 6: Token-driven rich layout and responsive fallbacks

**Files:**

- Modify: `DESIGN.md`
- Modify: `src/react-app/styles/tokens.css`
- Modify: `src/react-app/styles/workspace.css`
- Modify: `src/react-app/styles/events.css`
- Modify: `src/react-app/styles/theater.css`
- Modify: `src/react-app/styles/playback.css`
- Modify: `src/react-app/styles/responsive.css`
- Modify: `src/react-app/styles/mobile.css`
- Modify: `src/react-app/styles/accessibility.css`
- Modify: `tests/e2e/viewer.spec.ts`

**Interfaces:**

- Consumes: DOM/data attributes from Tasks 3–5.
- Produces: the 320px navigator, flexible stage, 240px inspector, compact playback/day deck, responsive grid/single-camera fallbacks, and forced-colors states.

- [ ] **Step 1: Add failing browser geometry and CJK assertions**

```ts
const regions = await page
  .locator(".event-navigator, .camera-stage, .event-inspector, .playback-deck")
  .evaluateAll((elements) =>
    elements.map((element) => element.getBoundingClientRect()),
  );
expect(
  regions.every(
    (bounds) =>
      bounds.left >= 0 &&
      bounds.right <= 1280 &&
      bounds.top >= 0 &&
      bounds.bottom <= 1000,
  ),
).toBe(true);
expect(
  await textSpansSingleLine(
    page.getByText("セントリー", { exact: true }).last(),
    "セントリー",
  ),
).toBe(true);
expect(
  await page.evaluate(
    () => document.documentElement.scrollWidth <= window.innerWidth,
  ),
).toBe(true);
expect(
  await page
    .locator(".camera-tile video")
    .evaluateAll((videos) =>
      videos.every((video) => getComputedStyle(video).objectFit === "contain"),
    ),
).toBe(true);
```

At 768px assert a 3×2 camera grid and collapsible inspector; at 375px assert one visible camera, six switcher buttons, and no new horizontal scroller beyond `.camera-switcher`.

- [ ] **Step 2: Run the E2E rich-layout case and confirm it fails before styling**

```bash
npm run build
npm run test:e2e -- --grep "rich investigation layout"
```

Expected: FAIL on geometry or missing selectors.

- [ ] **Step 3: Add exact layout tokens**

```css
--event-navigator-width: 320px;
--event-inspector-width: 240px;
--camera-stage-spatial-min: 720px;
--playback-deck-height: 196px;
```

Remove runtime use of `--library-rail-width` and `--event-list-width` once the consolidated navigator is wired.

- [ ] **Step 4: Implement desktop shell and evidence-stage placement**

```css
.library-workspace {
  display: grid;
  block-size: calc(100dvb - var(--app-header-height));
  grid-template-columns: var(--event-navigator-width) minmax(0, 1fr);
  overflow: hidden;
}

.theater-stage-row {
  display: grid;
  min-block-size: 0;
  grid-template-columns: minmax(0, 1fr) var(--event-inspector-width);
}
```

Keep the navigator, inspector, header, and playback controls light. Keep the camera stage and 24-hour timeline dark. Use existing colors, borders, radii, and spacing tokens without glass, glow, decorative gradient, or card shadows.

- [ ] **Step 5: Implement spatial placement over the same camera nodes**

```css
.camera-stage[data-view-mode="spatial"] {
  grid-template-areas:
    ". front ."
    "left-pillar vehicle right-pillar"
    "left-repeater back right-repeater";
}

.camera-tile[data-camera="front"] {
  grid-area: front;
}
.camera-tile[data-camera="back"] {
  grid-area: back;
}
.camera-tile[data-camera="left_pillar"] {
  grid-area: left-pillar;
}
.camera-tile[data-camera="left_repeater"] {
  grid-area: left-repeater;
}
.camera-tile[data-camera="right_pillar"] {
  grid-area: right-pillar;
}
.camera-tile[data-camera="right_repeater"] {
  grid-area: right-repeater;
}
```

Define the center vehicle image in `grid-area: vehicle`. Preserve canonical DOM order and ensure `object-fit: contain` remains unchanged.

- [ ] **Step 6: Protect record-row geometry and atomic categories**

```css
.event-card__heading {
  display: grid;
  grid-template-columns: minmax(0, 1fr) max-content;
}

.event-category {
  min-inline-size: max-content;
  white-space: nowrap;
}
```

Ellipsize time only when required; never split the icon from its Japanese category label.

- [ ] **Step 7: Implement tablet and mobile fallbacks**

At 640–1024px, stack the navigator, theater context, grid, collapsible inspector, controls, and compact day list. At 639px and below, preserve the one-visible-camera layout and local camera-switcher scroll. Hide the spatial switch below the minimum stage width; do not hide videos from the DOM.

```css
@media (max-width: 639px) {
  .camera-stage,
  .camera-stage[data-view-mode="spatial"] {
    grid-template-columns: 1fr;
    grid-template-rows: 1fr;
  }

  .camera-stage .camera-tile:not([data-active="true"]) {
    display: none;
  }
}
```

- [ ] **Step 8: Extend forced-colors and reduced-motion coverage**

```css
@media (forced-colors: active) {
  .theater-view-switch button[aria-pressed="true"],
  .day-timeline__marker[aria-pressed="true"] {
    outline: 3px solid Highlight;
    outline-offset: -3px;
  }
}
```

Keep the existing global reduced-motion rule and add no layout animation.

- [ ] **Step 9: Update the durable design-system document**

Document the new widths, component topology, spatial grid areas, dark-stage boundary, day-timeline semantics, 15-second pre-roll, atomic CJK labels, and responsive fallbacks in `DESIGN.md`.

- [ ] **Step 10: Run the targeted browser case, full component suite, typecheck, and lint**

```bash
npm run build
npm run test:e2e -- --grep "rich investigation layout"
npm test -- --run src/react-app/components/EventList.test.tsx src/react-app/components/EventInspector.test.tsx src/react-app/components/DayTimeline.test.tsx src/react-app/components/Theater.test.tsx
npm run typecheck
npm run lint
```

Expected: PASS.

- [ ] **Step 11: Commit the rich visual system**

```bash
git add DESIGN.md src/react-app/styles/tokens.css src/react-app/styles/workspace.css src/react-app/styles/events.css src/react-app/styles/theater.css src/react-app/styles/playback.css src/react-app/styles/responsive.css src/react-app/styles/mobile.css src/react-app/styles/accessibility.css tests/e2e/viewer.spec.ts
git commit -m "リッチ閲覧レイアウトを仕上げる"
```

---

### Task 7: Complete fixtures, regression gate, and visual evidence

**Files:**

- Modify: `tests/e2e/viewer.spec.ts`
- Create: `tests/e2e/fixtures/TeslaCam/SentryClips/event-sentry/event.json`
- Create: fresh ignored evidence under `.omo/evidence/teslacam-viewer/`

**Interfaces:**

- Consumes: all feature behavior and the existing synthetic MP4 bytes.
- Produces: complete automated regression coverage and fresh 1280/768/375 screenshots for independent review.

- [ ] **Step 1: Add synthetic Sentry metadata variants**

```json
{
  "timestamp": "2026-08-20T20:35:30",
  "reason": "sentry_aware_object_detection",
  "city": "Must not render",
  "street": "Must not render",
  "est_lat": "35.0000",
  "est_lon": "139.0000"
}
```

Keep the valid location-bearing fixture in the existing `2026-08-20_20-35-00` Sentry segment so its detection maps to elapsed 30 seconds and initial playback maps to 15 seconds. Keep under-15-second, malformed, and out-of-range variants in separate in-memory picker events. Do not add real TeslaCam media or metadata.

- [ ] **Step 2: Add end-to-end Sentry and no-wrap assertions**

```ts
await page.getByRole("button", { name: /セントリー/ }).click();
await expect(page.getByLabel("再生位置")).toHaveValue("15");
await expect(page.getByRole("button", { name: "再生" })).toBeVisible();
await expect(page.getByText("検知の15秒前から開始")).toBeVisible();
await expect(page.getByText("Must not render")).toHaveCount(0);
expect(
  await textSpansSingleLine(
    page.getByText("セントリー", { exact: true }).last(),
    "セントリー",
  ),
).toBe(true);
```

Cover list selection, timeline marker selection, next, previous, same-event reselection, repeated media ready, spatial/grid toggle, expanded/Escape, and URL request leakage.

- [ ] **Step 3: Run the immutable local quality gate**

```bash
npm run cf-typegen
git diff --exit-code -- worker-configuration.d.ts
npm run typecheck
npm run lint
npm test -- --run
npm run build
npm run test:e2e
npx wrangler deploy --dry-run
```

Expected: every command exits zero.

- [ ] **Step 4: Capture fresh synthetic screenshots**

Capture selected Sentry, missing camera, representative fallback, spatial, grid, and expanded states at 1280×1000, 768×1024, and 375×844. Store only synthetic fixture screenshots under `.omo/evidence/teslacam-viewer/screenshots/`.

- [ ] **Step 5: Compare the reference and implementation together**

Use a combined comparison input containing `.omo/evidence/teslacam-viewer/references/rich-view-reference.png` and the fresh 1280×1000 selected-Sentry screenshot. Check the four-region hierarchy, light/dark boundary, camera/vehicle relationship, event-row density, CJK wrapping, margins, padding, type weight, borders, radii, and media cropping. Fix visible mismatches and rerun the affected checks.

- [ ] **Step 6: Commit final fixture and test adjustments**

```bash
git add tests/e2e/viewer.spec.ts tests/e2e/fixtures/TeslaCam/SentryClips/event-sentry/event.json
git commit -m "リッチ閲覧モードの回帰検証を追加する"
```

- [ ] **Step 7: Run independent code and visual review on the committed full SHA**

Fix `review_sha="$(git rev-parse HEAD)"` before dispatching review. Require the code-quality reviewer and visual/CJK QA reviewer to report PASS against that same full SHA. Record their reports under `.omo/evidence/teslacam-viewer/`. If a review requires a source or test change, commit the fix, set a new `review_sha`, and rerun both reviews; Task 8 may consume only the latest SHA with both PASS verdicts and no later source/test commit.

---

### Task 8: Real SSD acceptance, GitHub, Cloudflare, and production verification

**Files:**

- No source files unless real-surface QA finds a defect.
- Create: fresh ignored QA/release evidence under `.omo/evidence/teslacam-viewer/`.

**Interfaces:**

- Consumes: the exact reviewed commit SHA from Task 7.
- Produces: real-SSD acceptance, pushed GitHub state, green GitHub Actions, a Cloudflare deployment for the same SHA, and live production evidence.

- [ ] **Step 1: Verify release identities and clean state**

```bash
gh auth status
npx wrangler whoami
git status --short --branch
git log --oneline --decorate -10
```

Confirm the intended GitHub repository is `kandotrun/teslacam-viewer`, the Cloudflare account owns `teslacam.2-38.com`, and the worktree contains no unrelated changes.

- [ ] **Step 2: Run real SSD QA in the user’s browser**

Open the verified local preview, select `/Volumes/TESLADRIVE/TeslaCam`, and inspect one RecentClips, SavedClips, and SentryClips event. Confirm the selected Sentry starts paused at the valid detection pre-roll, all camera edges remain visible, spatial/grid mode preserves playback, the event inspector shows only allowed facts, the timeline selects the correct event, and the event row keeps `セントリー` on one line.

Do not save screenshots containing real frames, filenames, location metadata, or volume paths. Inspect the browser Network panel and confirm no request URL or body contains TeslaCam media, Blob data, local filenames, timestamps, locations, or `/Volumes` paths.

- [ ] **Step 3: Verify the Safari directory-input fallback**

Select the same TeslaCam directory through the existing fallback and confirm the event list, one Sentry pre-roll, six-camera playback, and camera switching. Record only a text verdict without local media details.

- [ ] **Step 4: Re-run the release gate after any real-surface fix**

```bash
npm run typecheck
npm run lint
npm test -- --run
npm run build
npm run test:e2e
npx wrangler deploy --dry-run
git diff --check
```

Expected: PASS on the final commit candidate.

- [ ] **Step 5: Push the exact reviewed commit**

```bash
release_sha="$(git rev-parse HEAD)"
git push origin "$release_sha:refs/heads/main"
run_id="$(gh run list --repo kandotrun/teslacam-viewer --commit "$release_sha" --limit 1 --json databaseId --jq '.[0].databaseId')"
gh run watch "$run_id" --repo kandotrun/teslacam-viewer --exit-status
gh run view "$run_id" --repo kandotrun/teslacam-viewer --json headSha,conclusion,url
```

Require `headSha` to equal `release_sha` and `conclusion` to equal `success` before deploying.

- [ ] **Step 6: Deploy the same commit to Cloudflare**

```bash
release_sha="$(git rev-parse HEAD)"
release_tag="git-$(git rev-parse --short=7 HEAD)"
npm run build
npx wrangler deploy --message "Git commit $release_sha" --tag "$release_tag"
versions_json="$(npx wrangler versions list --json)"
deployment_json="$(npx wrangler deployments status --json)"
RELEASE_SHA="$release_sha" VERSIONS_JSON="$versions_json" DEPLOYMENT_JSON="$deployment_json" node --input-type=module -e '
const sourceSha = process.env.RELEASE_SHA ?? ""
const versions = JSON.parse(process.env.VERSIONS_JSON ?? "[]")
const deployment = JSON.parse(process.env.DEPLOYMENT_JSON ?? "{}")
const version = versions.find((entry) => entry.annotations?.["workers/message"] === `Git commit ${sourceSha}`)
if (version === undefined) throw new Error("Cloudflare version does not identify the release SHA")
if (version.annotations?.["workers/tag"] !== `git-${sourceSha.slice(0, 7)}`) throw new Error("Cloudflare version tag does not identify the release SHA")
const active = deployment.versions?.find((entry) => entry.version_id === version.id && entry.percentage === 100)
if (active === undefined) throw new Error("Cloudflare production deployment is not the release version at 100 percent")
console.log(JSON.stringify({ sourceSha, versionId: version.id, deploymentId: deployment.id }))
'
```

Record the printed full source SHA, Cloudflare version ID, and deployment ID. The assertion must pass before production is accepted.

- [ ] **Step 7: Verify production API, SPA, headers, and UI**

```bash
curl --fail --silent --show-error https://teslacam.2-38.com/api/health
curl --fail --silent --show-error --head https://teslacam.2-38.com/
curl --fail --silent --show-error https://teslacam.2-38.com/does-not-exist
```

Open `https://teslacam.2-38.com`, select the synthetic or real local folder in the user’s browser, and confirm the deployed rich UI, Sentry pre-roll, no-wrap event row, spatial/grid switch, inspector privacy, day timeline, and responsive behavior. Confirm unknown `/api/*` paths remain JSON 404 and unknown client paths return the SPA shell.

- [ ] **Step 8: Stop only when the production surface is observed working**

Report the final full commit SHA, GitHub Actions run, Cloudflare deployment identifier, automated gate counts, real-SSD text-only verdict, fresh synthetic screenshot paths, and any residual limitations.
