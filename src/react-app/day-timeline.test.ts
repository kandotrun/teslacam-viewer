import type { ClipCategory, TeslaCamEvent } from '../teslacam/types'

import { buildDayTimelineMarkers, selectTimelineDate } from './day-timeline'

function event(
  id: string,
  category: ClipCategory,
  timestamp: string,
  segmentTimestamps: readonly string[] = [timestamp],
): TeslaCamEvent {
  return {
    id,
    category,
    timestamp,
    segments: segmentTimestamps.map((segmentTimestamp) => ({
      timestamp: segmentTimestamp,
      cameras: {},
    })),
  }
}

describe('selectTimelineDate', () => {
  it('uses the selected event local date ahead of a filtered date', () => {
    const events = [event('selected', 'saved', '2026-08-24_12-00-00')]

    expect(selectTimelineDate(events, events[0], '2026-08-23')).toBe('2026-08-24')
  })

  it('uses the active date filter when no event is selected', () => {
    expect(selectTimelineDate([], undefined, '2028-02-29')).toBe('2028-02-29')
  })

  it('uses the first filtered event when selection and date filter are absent', () => {
    const events = [
      event('year-boundary', 'recent', '2027-01-01_00-00-00'),
      event('older', 'saved', '2026-12-31_23-59-59'),
    ]

    expect(selectTimelineDate(events, undefined, null)).toBe('2027-01-01')
  })

  it('returns null for an empty unfiltered collection', () => {
    expect(selectTimelineDate([], undefined, null)).toBeNull()
  })
})

describe('buildDayTimelineMarkers', () => {
  it('uses local wall-clock positions at the start and end of a day', () => {
    const events = [
      event('start', 'recent', '2026-08-24_00-00-00'),
      event('end', 'saved', '2026-08-24_23-59-59'),
    ]

    expect(buildDayTimelineMarkers(events, '2026-08-24')).toEqual([
      {
        eventId: 'start',
        category: 'recent',
        dateKey: '2026-08-24',
        timeLabel: '00:00:00',
        secondsOfDay: 0,
        positionPercent: 0,
      },
      {
        eventId: 'end',
        category: 'saved',
        dateKey: '2026-08-24',
        timeLabel: '23:59:59',
        secondsOfDay: 86_399,
        positionPercent: 100,
      },
    ])
  })

  it('orders simultaneous markers by category and then event id', () => {
    const events = [
      event('recent-event', 'recent', '2026-08-24_12-00-00'),
      event('saved-z', 'saved', '2026-08-24_12-00-00'),
      event('sentry-event', 'sentry', '2026-08-24_12-00-00'),
      event('saved-a', 'saved', '2026-08-24_12-00-00'),
    ]

    expect(buildDayTimelineMarkers(events, '2026-08-24').map((marker) => marker.eventId)).toEqual([
      'sentry-event',
      'saved-a',
      'saved-z',
      'recent-event',
    ])
  })

  it('assigns a recent session to its first segment local date', () => {
    const recent = event(
      'recent-session',
      'recent',
      '2026-08-25_00-00-20',
      ['2026-08-24_23-59-30', '2026-08-25_00-00-20'],
    )

    expect(buildDayTimelineMarkers([recent], '2026-08-24')[0]).toMatchObject({
      eventId: 'recent-session',
      dateKey: '2026-08-24',
      timeLabel: '23:59:30',
    })
    expect(buildDayTimelineMarkers([recent], '2026-08-25')).toEqual([])
  })

  it('keeps only the events supplied by an already filtered collection', () => {
    const matchingEvents = [event('leap-day', 'sentry', '2028-02-29_12-00-00')]
    const hiddenEvents = [event('other-day', 'saved', '2028-03-01_12-00-00')]

    expect(buildDayTimelineMarkers(matchingEvents, '2028-02-29')).toHaveLength(1)
    expect(buildDayTimelineMarkers(hiddenEvents, '2028-02-29')).toEqual([])
  })

  it('ignores malformed local dates and a missing timeline date', () => {
    const malformed = event('malformed', 'saved', '2026-02-29_12-00-00')

    expect(buildDayTimelineMarkers([malformed], '2026-02-29')).toEqual([])
    expect(buildDayTimelineMarkers([malformed], null)).toEqual([])
  })
})
