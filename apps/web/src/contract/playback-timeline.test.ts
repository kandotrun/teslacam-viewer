// @vitest-environment node
import { describe } from 'vitest'

import { eventDuration, locatePlaybackTime } from '../react-app/playback/timeline'
import type { TeslaCamEvent } from '../teslacam/types'
import { encodeNumber } from './adapters'
import { defineFixture, type FixtureCase } from './fixture-file'

function eventWithSegments(segmentCount: number): TeslaCamEvent {
  return {
    id: 'event',
    category: 'recent',
    timestamp: '2026-08-21_12-00-00',
    segments: Array.from({ length: segmentCount }, (_, index) => ({
      timestamp: `2026-08-21_12-${String(index).padStart(2, '0')}-00`,
      cameras: {},
    })),
  }
}

function durationCase(segmentCount: number): FixtureCase {
  return {
    name: `eventDuration ${segmentCount} segments`,
    fn: 'eventDuration',
    input: { segmentCount },
    expected: eventDuration(eventWithSegments(segmentCount)),
  }
}

function locateCase(segmentCount: number, elapsedSeconds: number): FixtureCase {
  const location = locatePlaybackTime(eventWithSegments(segmentCount), elapsedSeconds)
  return {
    name: `locatePlaybackTime ${segmentCount} segments at ${String(elapsedSeconds)}`,
    fn: 'locatePlaybackTime',
    input: { segmentCount, elapsedSeconds: encodeNumber(elapsedSeconds) },
    expected: { segmentIndex: location.segmentIndex, offsetSeconds: location.offsetSeconds },
  }
}

const elapsedValues = [
  0, 0.1, 30.5, 59.9, 59.999, 60, 60.000001, 65, 119.75, 120, 150, 179.9, 180, 180.0001, 200, 718.25, 720,
  -0.5, -3, Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY,
]

describe('contract: playback-timeline', () => {
  defineFixture({
    file: 'playback-timeline',
    module: 'playback-timeline',
    source: ['apps/web/src/react-app/playback/timeline.ts'],
    cases: () => [
      ...[0, 1, 2, 3, 12].map(durationCase),
      ...[0, 1, 2, 3, 12].flatMap((segmentCount) => elapsedValues.map((elapsed) => locateCase(segmentCount, elapsed))),
    ],
  })
})
