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
import { cameraIds, type CameraId, type ClipCategory } from '../teslacam/types'
import { defineFixture, type FixtureCase } from './fixture-file'

const categories: readonly ClipCategory[] = ['recent', 'saved', 'sentry']

function availability(missing: readonly CameraId[]) {
  return { available: cameraIds.length - missing.length, total: cameraIds.length, missing }
}

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
    source: ['apps/web/src/react-app/event-summary.ts', 'apps/web/src/react-app/event-filters.ts'],
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
    ],
  })
})
