import type { TeslaCamEvent } from '../../teslacam/types'

import { eventDuration, locatePlaybackTime } from './timeline'

function eventWithSegments(segmentCount: number): TeslaCamEvent {
  return {
    id: 'event',
    category: 'recent',
    timestamp: '2026-08-21_12-00-00',
    segments: Array.from({ length: segmentCount }, (_, segmentIndex) => ({
      timestamp: `2026-08-21_12-${String(segmentIndex).padStart(2, '0')}-00`,
      cameras: {},
    })),
  }
}

describe('eventDuration', () => {
  it('uses a nominal sixty-second window for every segment even when cameras are absent', () => {
    expect(eventDuration(eventWithSegments(2))).toBe(120)
  })

  it('returns zero for an event without segments', () => {
    expect(eventDuration(eventWithSegments(0))).toBe(0)
  })
})

describe('locatePlaybackTime', () => {
  it.each([
    [0, { segmentIndex: 0, offsetSeconds: 0 }],
    [30.5, { segmentIndex: 0, offsetSeconds: 30.5 }],
    [60, { segmentIndex: 1, offsetSeconds: 0 }],
    [120, { segmentIndex: 2, offsetSeconds: 0 }],
    [180, { segmentIndex: 2, offsetSeconds: 60 }],
  ])('locates start, middle, boundaries, and the final endpoint at %s seconds', (elapsedSeconds, expected) => {
    expect(locatePlaybackTime(eventWithSegments(3), elapsedSeconds)).toEqual(expected)
  })

  it('clamps negative and beyond-duration times into the event range', () => {
    const event = eventWithSegments(2)

    expect(locatePlaybackTime(event, -3)).toEqual({
      segmentIndex: 0,
      offsetSeconds: 0,
    })
    expect(locatePlaybackTime(event, 120.5)).toEqual({
      segmentIndex: 1,
      offsetSeconds: 60,
    })
  })

  it('keeps a twelve-segment event in its final nominal window', () => {
    expect(locatePlaybackTime(eventWithSegments(12), 718.25)).toEqual({
      segmentIndex: 11,
      offsetSeconds: 58.25,
    })
  })

  it('returns a neutral location without indexing an empty segment list', () => {
    expect(locatePlaybackTime(eventWithSegments(0), 14)).toEqual({
      segmentIndex: 0,
      offsetSeconds: 0,
    })
  })
})
