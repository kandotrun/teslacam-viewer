// @vitest-environment node
import { describe } from 'vitest'

import {
  adjacentTimelineDays,
  buildDayTimelineClusters,
  buildDayTimelineMarkers,
  buildTimelineDays,
  selectTimelineDate,
  type DayTimelineCluster,
  type DayTimelineMarker,
  type TimelineDay,
} from '../react-app/day-timeline'
import type { ClipCategory } from '../teslacam/types'
import { encodeNumber, eventFixtureJson, toTeslaCamEvent, type EventFixture } from './adapters'
import { defineFixture, type FixtureCase, type JsonValue } from './fixture-file'

function event(id: string, category: ClipCategory, timestamp: string, segments: readonly string[] = [timestamp]): EventFixture {
  return { id, category, timestamp, segments: segments.map((segment) => ({ timestamp: segment, cameras: ['front'] })) }
}

function markerJson(marker: DayTimelineMarker): JsonValue {
  return {
    eventId: marker.eventId,
    category: marker.category,
    dateKey: marker.dateKey,
    timeLabel: marker.timeLabel,
    secondsOfDay: marker.secondsOfDay,
    positionPercent: marker.positionPercent,
  }
}

function dayJson(day: TimelineDay | undefined): JsonValue {
  return day === undefined ? null : { dateKey: day.dateKey, eventIds: [...day.eventIds] }
}

function clusterJson(cluster: DayTimelineCluster): JsonValue {
  return {
    id: cluster.id,
    count: cluster.count,
    eventIds: cluster.markers.map((marker) => marker.eventId),
    startTimeLabel: cluster.startTimeLabel,
    endTimeLabel: cluster.endTimeLabel,
    startSecondsOfDay: cluster.startSecondsOfDay,
    endSecondsOfDay: cluster.endSecondsOfDay,
    durationSeconds: cluster.durationSeconds,
    startPositionPercent: cluster.startPositionPercent,
    endPositionPercent: cluster.endPositionPercent,
  }
}

function selectCase(name: string, events: readonly EventFixture[], selectedEventId: string | null, filterDateKey: string | null): FixtureCase {
  const realEvents = events.map(toTeslaCamEvent)
  return {
    name: `selectTimelineDate ${name}`,
    fn: 'selectTimelineDate',
    input: { events: events.map(eventFixtureJson), selectedEventId, filterDateKey },
    expected: selectTimelineDate(realEvents, realEvents.find((candidate) => candidate.id === selectedEventId), filterDateKey),
  }
}

function markersCase(name: string, events: readonly EventFixture[], dateKey: string | null): FixtureCase {
  return {
    name: `buildDayTimelineMarkers ${name}`,
    fn: 'buildDayTimelineMarkers',
    input: { events: events.map(eventFixtureJson), dateKey },
    expected: buildDayTimelineMarkers(events.map(toTeslaCamEvent), dateKey).map(markerJson),
  }
}

function daysCase(name: string, events: readonly EventFixture[]): FixtureCase {
  return {
    name: `buildTimelineDays ${name}`,
    fn: 'buildTimelineDays',
    input: { events: events.map(eventFixtureJson) },
    expected: buildTimelineDays(events.map(toTeslaCamEvent)).map(dayJson),
  }
}

function adjacentCase(name: string, events: readonly EventFixture[], currentDateKey: string | null): FixtureCase {
  const days = buildTimelineDays(events.map(toTeslaCamEvent))
  const adjacency = adjacentTimelineDays(days, currentDateKey)
  return {
    name: `adjacentTimelineDays ${name}`,
    fn: 'adjacentTimelineDays',
    input: { days: days.map(dayJson), currentDateKey },
    expected: { previous: dayJson(adjacency.previous), next: dayJson(adjacency.next) },
  }
}

function clustersCase(name: string, events: readonly EventFixture[], dateKey: string | null, trackWidthPixels: number): FixtureCase {
  return {
    name: `buildDayTimelineClusters ${name} at ${String(trackWidthPixels)}`,
    fn: 'buildDayTimelineClusters',
    input: { events: events.map(eventFixtureJson), dateKey, trackWidthPixels: encodeNumber(trackWidthPixels) },
    expected: buildDayTimelineClusters(events.map(toTeslaCamEvent), dateKey, trackWidthPixels).map(clusterJson),
  }
}

const day = '2026-08-24'

const sameSecond = [
  event('recent-event', 'recent', '2026-08-24_12-00-00'),
  event('saved-z', 'saved', '2026-08-24_12-00-00'),
  event('sentry-event', 'sentry', '2026-08-24_12-00-00'),
  event('saved-a', 'saved', '2026-08-24_12-00-00'),
]

const recentSession = event('recent-session', 'recent', '2026-08-25_00-00-20', ['2026-08-24_23-59-30', '2026-08-25_00-00-20'])

const multiDay = [
  event('late', 'saved', '2026-08-25_23-00-00'),
  event('first', 'sentry', '2026-08-23_10-00-00'),
  event('middle', 'saved', '2026-08-24_12-00-00'),
  recentSession,
  event('invalid', 'saved', '2026-02-29_12-00-00'),
]

const threshold = [
  event('a', 'saved', '2026-08-24_00-00-00'),
  event('b', 'saved', '2026-08-24_01-38-59'),
  event('c', 'saved', '2026-08-24_03-17-59'),
  event('d', 'saved', '2026-08-24_08-00-00'),
]

const chained = [
  event('chain-1', 'recent', '2026-08-24_10-00-00'),
  event('chain-2', 'recent', '2026-08-24_11-23-20'),
  event('chain-3', 'recent', '2026-08-24_12-46-40'),
]

const evening = [
  event('sentry-2026-08-24_20-35-00-1', 'sentry', '2026-08-24_20-35-00'),
  event('saved-2026-08-24_20-40-00-1', 'saved', '2026-08-24_20-40-00'),
  event('recent-2026-08-24_20-45-00-1', 'recent', '2026-08-24_20-45-00'),
  event('saved-2026-08-24_08-00-00-2', 'saved', '2026-08-24_08-00-00'),
  event('sentry-2026-08-24_23-59-59-2', 'sentry', '2026-08-24_23-59-59'),
  event('recent-2026-08-24_00-00-00-2', 'recent', '2026-08-24_00-00-00'),
]

const widths = [640, 340, 1000, 1, 0, -1, Number.NaN, Number.POSITIVE_INFINITY]
const measuredWidths = [640, 340, 1000, 1]

describe('contract: day-timeline', () => {
  defineFixture({
    file: 'day-timeline',
    module: 'day-timeline',
    source: ['apps/web/src/react-app/day-timeline.ts', 'apps/web/src/react-app/local-timestamp.ts'],
    cases: () => [
      selectCase('selected event wins over the filter', [event('selected', 'saved', '2026-08-24_12-00-00')], 'selected', '2026-08-23'),
      selectCase('date filter without selection', [], null, '2028-02-29'),
      selectCase('invalid date filter falls back to events', [event('only', 'saved', '2026-08-24_12-00-00')], null, '2026-02-30'),
      selectCase('first parsable event', [event('bad', 'saved', '2026-13-01_00-00-00'), event('year-boundary', 'recent', '2027-01-01_00-00-00'), event('older', 'saved', '2026-12-31_23-59-59')], null, null),
      selectCase('empty collection', [], null, null),
      selectCase('recent session uses its first segment', [recentSession], 'recent-session', null),
      selectCase('unknown selected id', [event('only', 'saved', '2026-08-24_12-00-00')], 'missing', '2026-08-01'),
      markersCase('day bounds', [event('start', 'recent', '2026-08-24_00-00-00'), event('end', 'saved', '2026-08-24_23-59-59')], day),
      markersCase('same second ordering', sameSecond, day),
      markersCase('recent session on its first date', [recentSession], day),
      markersCase('recent session not on its last date', [recentSession], '2026-08-25'),
      markersCase('leap day', [event('leap-day', 'sentry', '2028-02-29_12-00-00')], '2028-02-29'),
      markersCase('other day hidden', [event('other-day', 'saved', '2028-03-01_12-00-00')], '2028-02-29'),
      markersCase('malformed event date', [event('malformed', 'saved', '2026-02-29_12-00-00')], '2026-02-29'),
      markersCase('null date', [event('malformed', 'saved', '2026-02-29_12-00-00')], null),
      markersCase('invalid date key', evening, '2026-8-24'),
      markersCase('evening', evening, day),
      daysCase('multiple days', multiDay),
      daysCase('evening', evening),
      daysCase('empty', []),
      adjacentCase('between days', [event('first', 'sentry', '2026-08-23_10-00-00'), event('middle', 'saved', '2026-08-25_12-00-00'), event('last', 'recent', '2026-08-28_15-00-00')], '2026-08-26'),
      adjacentCase('on a recorded day', multiDay, '2026-08-24'),
      adjacentCase('first day', multiDay, '2026-08-23'),
      adjacentCase('last day', multiDay, '2026-08-25'),
      adjacentCase('invalid current date', multiDay, '2026-02-30'),
      adjacentCase('null current date', multiDay, null),
      adjacentCase('across a year', [event('old', 'saved', '2026-12-31_10-00-00'), event('new', 'saved', '2027-01-02_10-00-00')], '2027-01-01'),
      ...widths.map((width) => clustersCase('threshold', threshold, day, width)),
      ...measuredWidths.map((width) => clustersCase('chained', chained, day, width)),
      ...measuredWidths.map((width) => clustersCase('evening', evening, day, width)),
      clustersCase('same second', sameSecond, day, 640),
      clustersCase('near pair', [event('near-a', 'sentry', '2026-08-24_10-00-00'), event('near-b', 'saved', '2026-08-24_10-00-30'), event('separate', 'recent', '2026-08-24_12-00-00')], day, 1000),
      clustersCase('no date', evening, null, 640),
      clustersCase('empty day', evening, '2026-08-23', 640),
    ],
  })
})
