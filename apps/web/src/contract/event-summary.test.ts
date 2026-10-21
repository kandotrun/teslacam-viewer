// @vitest-environment node
import { describe } from 'vitest'

import {
  cameraAvailability,
  cameraAvailabilityLabel,
  eventDurationLabel,
  missingCameraLabel,
  segmentCountLabel,
} from '../react-app/event-summary'
import { eventDuration } from '../react-app/playback/timeline'
import { cameraIds, type CameraId } from '../teslacam/types'
import { eventFixtureJson, toTeslaCamEvent, type EventFixture } from './adapters'
import { defineFixture, type FixtureCase } from './fixture-file'

function event(id: string, segments: readonly (readonly CameraId[])[]): EventFixture {
  return {
    id,
    category: 'saved',
    timestamp: '2026-08-20_20-40-00',
    segments: segments.map((cameras, index) => ({
      timestamp: `2026-08-20_20-${String(40 + index).padStart(2, '0')}-00`,
      cameras,
    })),
  }
}

const events: readonly EventFixture[] = [
  event('six cameras', [cameraIds]),
  event('front only', [['front']]),
  event('right repeater missing', [cameraIds.filter((camera) => camera !== 'right_repeater')]),
  event('no cameras', [[]]),
  event('no segments', []),
  event('full then front only', [cameraIds, ['front']]),
  event('different cameras per segment', [['front', 'back'], ['left_pillar', 'right_pillar']]),
  event('back and repeaters', [['back', 'left_repeater', 'right_repeater']]),
  event('eleven segments', Array.from({ length: 11 }, () => cameraIds)),
]

/** Long events only pin the duration helpers, which keeps the fixture small. */
const longEvents: readonly EventFixture[] = [
  event('sixty segments', Array.from({ length: 60 }, () => [])),
  event('sixty-one segments', Array.from({ length: 61 }, () => [])),
]

function eventCases(fixture: EventFixture): FixtureCase[] {
  const teslaCamEvent = toTeslaCamEvent(fixture)
  const availability = cameraAvailability(teslaCamEvent)
  const input = { event: eventFixtureJson(fixture) }
  return [
    {
      name: `cameraAvailability ${fixture.id}`,
      fn: 'cameraAvailability',
      input,
      expected: { available: availability.available, total: availability.total, missing: [...availability.missing] },
    },
    { name: `cameraAvailabilityLabel ${fixture.id}`, fn: 'cameraAvailabilityLabel', input, expected: cameraAvailabilityLabel(availability) },
    { name: `missingCameraLabel ${fixture.id}`, fn: 'missingCameraLabel', input, expected: missingCameraLabel(availability) },
    { name: `eventDuration ${fixture.id}`, fn: 'eventDuration', input, expected: eventDuration(teslaCamEvent) },
    { name: `eventDurationLabel ${fixture.id}`, fn: 'eventDurationLabel', input, expected: eventDurationLabel(teslaCamEvent) },
    { name: `segmentCountLabel ${fixture.id}`, fn: 'segmentCountLabel', input, expected: segmentCountLabel(teslaCamEvent) },
  ]
}

describe('contract: event-summary', () => {
  defineFixture({
    file: 'event-summary',
    module: 'event-summary',
    source: ['apps/web/src/react-app/event-summary.ts', 'apps/web/src/react-app/playback/timeline.ts'],
    cases: () => [
      ...events.flatMap(eventCases),
      ...longEvents.flatMap((fixture) => eventCases(fixture).filter((fixtureCase) => fixtureCase.fn.startsWith('eventDuration'))),
    ],
  })
})
