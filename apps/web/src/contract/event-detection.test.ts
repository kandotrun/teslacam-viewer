// @vitest-environment node
import { describe } from 'vitest'

import {
  detectionSeconds,
  sentryInitialElapsedSeconds,
  sentryPlaybackStartLabel,
} from '../react-app/event-detection'
import type { ClipCategory, EventMetadata } from '../teslacam/types'
import { eventFixtureJson, toTeslaCamEvent, type EventFixture } from './adapters'
import { defineFixture, type FixtureCase } from './fixture-file'

type DetectionEvent = {
  readonly name: string
  readonly category?: ClipCategory
  readonly segments: readonly string[]
  readonly detection?: string
  readonly metadata?: EventMetadata
}

function fixture(definition: DetectionEvent): EventFixture {
  const metadata = definition.metadata ?? (definition.detection === undefined ? undefined : { timestamp: definition.detection })
  return {
    id: 'detection-event',
    category: definition.category ?? 'sentry',
    timestamp: definition.segments[0] ?? '2026-08-24_12-00-00',
    segments: definition.segments.map((timestamp) => ({ timestamp, cameras: ['front'] })),
    ...(metadata === undefined ? {} : { metadata }),
  }
}

function detectionCases(definition: DetectionEvent): FixtureCase[] {
  const eventFixture = fixture(definition)
  const event = toTeslaCamEvent(eventFixture)
  const input = { event: eventFixtureJson(eventFixture) }
  return [
    { name: `detectionSeconds ${definition.name}`, fn: 'detectionSeconds', input, expected: detectionSeconds(event) },
    { name: `sentryInitialElapsedSeconds ${definition.name}`, fn: 'sentryInitialElapsedSeconds', input, expected: sentryInitialElapsedSeconds(event) },
    { name: `sentryPlaybackStartLabel ${definition.name}`, fn: 'sentryPlaybackStartLabel', input, expected: sentryPlaybackStartLabel(event) },
  ]
}

const one = ['2026-08-24_12-00-00']
const two = ['2026-08-24_12-00-00', '2026-08-24_12-01-00']

const definitions: readonly DetectionEvent[] = [
  { name: 'first ten seconds', segments: one, detection: '2026-08-24T12:00:05' },
  { name: 'second segment', segments: two, detection: '2026-08-24T12:01:20' },
  { name: 'gap between segments', segments: ['2026-08-24_12-00-00', '2026-08-24_12-01-11'], detection: '2026-08-24T12:01:05' },
  { name: 'overlapping segments pick the earliest', segments: ['2026-08-24_12-00-00', '2026-08-24_12-00-50'], detection: '2026-08-24T12:00:55' },
  { name: 'exact non-final boundary', segments: two, detection: '2026-08-24T12:01:00' },
  { name: 'final segment endpoint', segments: two, detection: '2026-08-24T12:02:00' },
  { name: 'after the final endpoint', segments: two, detection: '2026-08-24T12:02:01' },
  { name: 'before the first segment', segments: two, detection: '2026-08-24T11:59:59' },
  { name: 'local midnight with Z', segments: ['2026-08-24_23-59-30', '2026-08-25_00-00-30'], detection: '2026-08-25T00:00:45Z' },
  { name: 'malformed', segments: one, detection: 'invalid' },
  { name: 'offset suffix', segments: one, detection: '2026-08-24T12:00:30+09:00' },
  { name: 'out of range single segment', segments: one, detection: '2026-08-24T12:01:01' },
  { name: 'milliseconds are dropped', segments: one, detection: '2026-08-24T12:00:05.900' },
  { name: 'detection at 14 seconds', segments: one, detection: '2026-08-24T12:00:14' },
  { name: 'detection at 15 seconds', segments: one, detection: '2026-08-24T12:00:15' },
  { name: 'detection at 16 seconds', segments: one, detection: '2026-08-24T12:00:16' },
  { name: 'detection at the start', segments: one, detection: '2026-08-24T12:00:00' },
  { name: 'pre-roll crosses into the previous segment', segments: two, detection: '2026-08-24T12:01:10' },
  { name: 'e2e sentry fixture', segments: ['2026-08-20_20-35-00'], detection: '2026-08-20T20:35:30' },
  { name: 'saved with valid detection', category: 'saved', segments: one, detection: '2026-08-24T12:00:30' },
  { name: 'recent with valid detection', category: 'recent', segments: one, detection: '2026-08-24T12:00:30' },
  { name: 'no metadata', segments: one },
  { name: 'metadata without a usable timestamp', segments: one, metadata: { timestamp: '', reason: 'manual_save' } },
  { name: 'no segments', segments: [], detection: '2026-08-24T12:00:05' },
  { name: 'invalid segment timestamp is skipped', segments: ['2026-02-30_12-00-00', '2026-08-24_12-00-00'], detection: '2026-08-24T12:00:10' },
  { name: 'three segments, last one', segments: ['2026-08-24_12-00-00', '2026-08-24_12-01-00', '2026-08-24_12-02-00'], detection: '2026-08-24T12:02:40' },
  { name: 'real cadence 61 seconds', segments: ['2026-08-24_12-00-00', '2026-08-24_12-01-01'], detection: '2026-08-24T12:01:00' },
]

describe('contract: event-detection', () => {
  defineFixture({
    file: 'event-detection',
    module: 'event-detection',
    source: ['apps/web/src/react-app/event-detection.ts', 'apps/web/src/react-app/local-timestamp.ts'],
    cases: () => definitions.flatMap(detectionCases),
  })
})
