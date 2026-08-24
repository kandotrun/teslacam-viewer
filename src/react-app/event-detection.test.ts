import type { TeslaCamEvent } from '../teslacam/types'
import {
  detectionSeconds,
  sentryInitialElapsedSeconds,
  sentryPlaybackStartLabel,
} from './event-detection'

type SentryEventOptions = {
  readonly segments: readonly string[]
  readonly detection: string
}

function sentryEvent({ segments, detection }: SentryEventOptions): TeslaCamEvent {
  const firstSegment = segments.at(0)
  if (firstSegment === undefined) throw new Error('a sentry event requires at least one segment')
  return {
    id: 'sentry-event',
    category: 'sentry',
    timestamp: firstSegment,
    metadata: { timestamp: detection },
    segments: segments.map((timestamp) => ({ timestamp, cameras: {} })),
  }
}

describe('detectionSeconds', () => {
  it('maps a detection in the first ten seconds to event elapsed time', () => {
    const event = sentryEvent({
      segments: ['2026-08-24_12-00-00'],
      detection: '2026-08-24T12:00:05',
    })

    expect(detectionSeconds(event)).toBe(5)
  })

  it('maps a detection in the second segment to its event elapsed position', () => {
    const event = sentryEvent({
      segments: ['2026-08-24_12-00-00', '2026-08-24_12-01-00'],
      detection: '2026-08-24T12:01:20',
    })

    expect(detectionSeconds(event)).toBe(80)
  })

  it('returns null when detection is in a gap between segments', () => {
    const event = sentryEvent({
      segments: ['2026-08-24_12-00-00', '2026-08-24_12-01-11'],
      detection: '2026-08-24T12:01:05',
    })

    expect(detectionSeconds(event)).toBeNull()
  })

  it('selects the earliest segment when segment time ranges overlap', () => {
    const event = sentryEvent({
      segments: ['2026-08-24_12-00-00', '2026-08-24_12-00-50'],
      detection: '2026-08-24T12:00:55',
    })

    expect(detectionSeconds(event)).toBe(55)
  })

  it('uses the following segment at an exact non-final boundary', () => {
    const event = sentryEvent({
      segments: ['2026-08-24_12-00-00', '2026-08-24_12-01-00'],
      detection: '2026-08-24T12:01:00',
    })

    expect(detectionSeconds(event)).toBe(60)
  })

  it('accepts the final segment endpoint', () => {
    const event = sentryEvent({
      segments: ['2026-08-24_12-00-00', '2026-08-24_12-01-00'],
      detection: '2026-08-24T12:02:00',
    })

    expect(detectionSeconds(event)).toBe(120)
  })

  it('handles local midnight without timezone conversion', () => {
    const event = sentryEvent({
      segments: ['2026-08-24_23-59-30', '2026-08-25_00-00-30'],
      detection: '2026-08-25T00:00:45Z',
    })

    expect(detectionSeconds(event)).toBe(75)
  })

  it('returns null for malformed, offset, and out-of-range metadata', () => {
    const segments = ['2026-08-24_12-00-00']

    expect(detectionSeconds(sentryEvent({ segments, detection: 'invalid' }))).toBeNull()
    expect(detectionSeconds(sentryEvent({ segments, detection: '2026-08-24T12:00:30+09:00' }))).toBeNull()
    expect(detectionSeconds(sentryEvent({ segments, detection: '2026-08-24T12:01:01' }))).toBeNull()
  })
})

describe('sentry pre-roll', () => {
  it('starts Sentry playback fifteen seconds before detection and labels that local moment', () => {
    const event = sentryEvent({
      segments: ['2026-08-24_12-00-00', '2026-08-24_12-01-00'],
      detection: '2026-08-24T12:01:20',
    })

    expect(sentryInitialElapsedSeconds(event)).toBe(65)
    expect(sentryPlaybackStartLabel(event)).toBe('2026-08-24 12:01:05')
  })

  it('keeps non-Sentry events at the beginning even when metadata has a detection time', () => {
    const event: TeslaCamEvent = {
      ...sentryEvent({
        segments: ['2026-08-24_12-00-00'],
        detection: '2026-08-24T12:00:30',
      }),
      category: 'saved',
    }

    expect(sentryInitialElapsedSeconds(event)).toBe(0)
    expect(sentryPlaybackStartLabel(event)).toBeNull()
  })
})
