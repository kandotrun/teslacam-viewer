// @vitest-environment node
import { describe } from 'vitest'

import { categoryLabel, eventTime } from '../react-app/event-filters'
import {
  cameraAvailabilityLabel,
  cameraLabels,
  formatDuration,
  missingCameraLabel,
  segmentCountLabel,
} from '../react-app/event-summary'
import {
  cameraCountText,
  coordinatesText,
  dayTitle,
  detectionFact,
  detectionSentence,
  locationText,
  missingCamerasText,
  playbackStartText,
  scanProgressCopy,
  scrubberValueText,
  sentryDetection,
  shortCategoryLabels,
  startClock,
  transportTimeLabel,
  warningText,
} from '../react-app/presentation/event-copy'
import { cameraIds, type CameraId, type ClipCategory, type EventMetadata, type ScanWarning } from '../teslacam/types'
import { eventFixtureJson, metadataJson, toTeslaCamEvent, type EventFixture } from './adapters'
import { defineFixture, type FixtureCase } from './fixture-file'

const categories: readonly ClipCategory[] = ['recent', 'saved', 'sentry']

function availability(missing: readonly CameraId[]) {
  return { available: cameraIds.length - missing.length, total: cameraIds.length, missing }
}

function sentryEvent(name: string, segments: readonly string[], detection: string | null, reason?: string): EventFixture {
  return {
    id: name,
    category: 'sentry',
    timestamp: segments[0] ?? '2026-08-20_20-35-00',
    segments: segments.map((timestamp) => ({ timestamp, cameras: ['front'] })),
    ...(detection === null ? {} : { metadata: { timestamp: detection, ...(reason === undefined ? {} : { reason }) } }),
  }
}

const detectionEvents: readonly EventFixture[] = [
  sentryEvent('object detection', ['2026-08-20_20-35-00'], '2026-08-20T20:35:30', 'sentry_aware_object_detection'),
  sentryEvent('acceleration', ['2026-08-20_20-35-00'], '2026-08-20T20:35:30', 'sentry_aware_accel_0.52'),
  sentryEvent('door handle', ['2026-08-20_20-35-00'], '2026-08-20T20:35:30', 'sentry_locked_handle_pulled'),
  sentryEvent('unknown reason', ['2026-08-20_20-35-00'], '2026-08-20T20:35:30', 'user_interaction_honk'),
  sentryEvent('no reason', ['2026-08-20_20-35-00'], '2026-08-20T20:35:30'),
  sentryEvent('second segment', ['2026-08-24_12-00-00', '2026-08-24_12-01-00'], '2026-08-24T12:01:20', 'sentry_aware_object_detection'),
  sentryEvent('early detection', ['2026-08-24_12-00-00'], '2026-08-24T12:00:14'),
  sentryEvent('detection at 15 seconds', ['2026-08-24_12-00-00'], '2026-08-24T12:00:15'),
  sentryEvent('across midnight', ['2026-08-24_23-59-30', '2026-08-25_00-00-30'], '2026-08-25T00:00:45Z'),
  sentryEvent('outside the event', ['2026-08-24_12-00-00'], '2026-08-24T12:01:01'),
  sentryEvent('no metadata', ['2026-08-24_12-00-00'], null),
  { ...sentryEvent('saved event', ['2026-08-24_12-00-00'], '2026-08-24T12:00:30'), category: 'saved' },
]

function detectionCases(event: EventFixture): FixtureCase[] {
  const realEvent = toTeslaCamEvent(event)
  const detection = sentryDetection(realEvent)
  const input = { event: eventFixtureJson(event) }
  return [
    { name: `detectionFact ${event.id}`, fn: 'detectionFact', input, expected: detection === null ? null : detectionFact(detection) },
    { name: `detectionSentence ${event.id}`, fn: 'detectionSentence', input, expected: detection === null ? null : detectionSentence(detection) },
    { name: `playbackStartText ${event.id}`, fn: 'playbackStartText', input, expected: playbackStartText(realEvent) },
  ]
}

const metadataSamples: readonly (EventMetadata | null)[] = [
  { timestamp: 't', city: ' Sample city ', street: 'Sample street', est_lat: '35.0000', est_lon: ' 139.0000 ' },
  { timestamp: 't', street: 'Sample street', est_lat: '35.0000' },
  { timestamp: 't', city: ' ', street: '', est_lat: ' ', est_lon: '139' },
  { timestamp: 't' },
  null,
]

const warnings: readonly ScanWarning[] = [
  { kind: 'empty-media-file', count: 1 },
  { kind: 'invalid-metadata-file', count: 2 },
  { kind: 'unknown-file', count: 1234 },
  { kind: 'encrypted-file', count: 3 },
]

const missingSets: readonly (readonly CameraId[])[] = [
  [],
  ['right_repeater'],
  ['back', 'left_pillar', 'left_repeater', 'right_pillar', 'right_repeater'],
  cameraIds,
]

describe('contract: copy-format', () => {
  defineFixture({
    file: 'copy-format',
    module: 'copy',
    source: [
      'apps/web/src/react-app/event-summary.ts',
      'apps/web/src/react-app/event-filters.ts',
      'apps/web/src/react-app/presentation/event-copy.ts',
    ],
    cases: () => [
      ...cameraIds.map((camera): FixtureCase => ({
        name: `cameraLabel ${camera}`,
        fn: 'cameraLabel',
        input: { camera },
        expected: cameraLabels[camera],
      })),
      ...categories.map((category): FixtureCase => ({
        name: `categoryLabel ${category}`,
        fn: 'categoryLabel',
        input: { category },
        expected: categoryLabel(category),
      })),
      ...[0, 1, 15, 59, 60, 61, 119, 120, 660, 3599, 3600, 3660, 7200, 7260, 86_400, 90_061].map((seconds): FixtureCase => ({
        name: `formatDuration ${seconds}`,
        fn: 'formatDuration',
        input: { seconds },
        expected: formatDuration(seconds),
      })),
      ...missingSets.map((missing): FixtureCase => ({
        name: `cameraAvailabilityLabel ${cameraIds.length - missing.length} available`,
        fn: 'cameraAvailabilityLabel',
        input: { missing: [...missing] },
        expected: cameraAvailabilityLabel(availability(missing)),
      })),
      ...missingSets.map((missing): FixtureCase => ({
        name: `missingCameraLabel ${missing.length} missing`,
        fn: 'missingCameraLabel',
        input: { missing: [...missing] },
        expected: missingCameraLabel(availability(missing)),
      })),
      ...[0, 1, 11].map((segmentCount): FixtureCase => ({
        name: `segmentCountLabel ${segmentCount}`,
        fn: 'segmentCountLabel',
        input: { segmentCount },
        expected: segmentCountLabel({
          id: 'event',
          category: 'saved',
          timestamp: '2026-08-20_20-40-00',
          segments: Array.from({ length: segmentCount }, () => ({ timestamp: '2026-08-20_20-40-00', cameras: {} })),
        }),
      })),
      ...['2026-08-20_20-45-00', '2027-01-01_00-00-00'].map((timestamp): FixtureCase => ({
        name: `eventTime ${timestamp}`,
        fn: 'eventTime',
        input: { timestamp },
        expected: eventTime(timestamp),
      })),
      ...categories.map((category): FixtureCase => ({
        name: `shortCategoryLabel ${category}`,
        fn: 'shortCategoryLabel',
        input: { category },
        expected: shortCategoryLabels[category],
      })),
      ...['2026-08-20_20-45-00', '2026-08-20_09-05-59'].map((timestamp): FixtureCase => ({
        name: `startClock ${timestamp}`,
        fn: 'startClock',
        input: { timestamp },
        expected: startClock(timestamp),
      })),
      ...missingSets.map((missing): FixtureCase => ({
        name: `cameraCountText ${cameraIds.length - missing.length} available`,
        fn: 'cameraCountText',
        input: { missing: [...missing] },
        expected: cameraCountText(availability(missing)),
      })),
      ...missingSets.map((missing): FixtureCase => ({
        name: `missingCamerasText ${missing.length} missing`,
        fn: 'missingCamerasText',
        input: { missing: [...missing] },
        expected: missingCamerasText(availability(missing)),
      })),
      ...detectionEvents.flatMap(detectionCases),
      ...metadataSamples.flatMap((metadata, index): FixtureCase[] => {
        const input = { metadata: metadata === null ? null : metadataJson(metadata) }
        return [
          { name: `locationText sample ${index}`, fn: 'locationText', input, expected: locationText(metadata ?? undefined) },
          { name: `coordinatesText sample ${index}`, fn: 'coordinatesText', input, expected: coordinatesText(metadata ?? undefined) },
        ]
      }),
      ...['2026-08-20', '0001-01-01', '2026-10-18', '2028-02-29', '2026-02-29'].map((dateKey): FixtureCase => {
        const title = dayTitle(dateKey)
        return {
          name: `dayTitle ${dateKey}`,
          fn: 'dayTitle',
          input: { dateKey },
          expected: title === null ? null : { date: title.date, weekday: title.weekday },
        }
      }),
      ...[0, 0.9, 15.4, 59.99, 60, 65, 300, 3_599, 3_600, -3].map((seconds): FixtureCase => ({
        name: `transportTimeLabel ${seconds}`,
        fn: 'transportTimeLabel',
        input: { seconds },
        expected: transportTimeLabel(seconds),
      })),
      ...[[0, 60], [15, 60], [15.9, 60], [60, 120], [65, 125], [0, 0]].map(([elapsedSeconds = 0, durationSeconds = 0]): FixtureCase => ({
        name: `scrubberValueText ${elapsedSeconds} of ${durationSeconds}`,
        fn: 'scrubberValueText',
        input: { elapsedSeconds, durationSeconds },
        expected: scrubberValueText(elapsedSeconds, durationSeconds),
      })),
      ...[[0, 0], [250, 250], [1250, 1250], [250, 500], [12_345, 100_000]].map(([processed = 0, total = 0]): FixtureCase => {
        const copy = scanProgressCopy({ processed, total })
        return {
          name: `scanProgressCopy ${processed} of ${total}`,
          fn: 'scanProgressCopy',
          input: { processed, total },
          expected: { count: copy.count, label: copy.label },
        }
      }),
      ...warnings.map((warning): FixtureCase => ({
        name: `warningText ${warning.kind}`,
        fn: 'warningText',
        input: { kind: warning.kind, count: warning.count },
        expected: warningText(warning),
      })),
    ],
  })
})
