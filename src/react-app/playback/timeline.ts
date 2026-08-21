import type { TeslaCamEvent } from '../../teslacam/types'

const segmentDurationSeconds = 60

export type PlaybackLocation = {
  readonly segmentIndex: number
  readonly offsetSeconds: number
}

export function eventDuration(event: TeslaCamEvent): number {
  return event.segments.length * segmentDurationSeconds
}

export function locatePlaybackTime(
  event: TeslaCamEvent,
  elapsedSeconds: number,
): PlaybackLocation {
  const duration = eventDuration(event)
  if (duration === 0) return { segmentIndex: 0, offsetSeconds: 0 }

  const clampedSeconds = Math.min(Math.max(elapsedSeconds, 0), duration)
  const finalSegmentIndex = event.segments.length - 1
  const segmentIndex = Math.min(
    Math.floor(clampedSeconds / segmentDurationSeconds),
    finalSegmentIndex,
  )

  return {
    segmentIndex,
    offsetSeconds: clampedSeconds - segmentIndex * segmentDurationSeconds,
  }
}
