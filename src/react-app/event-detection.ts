import type { TeslaCamEvent } from '../teslacam/types'
import {
  formatFilenameTimestampAtOffset,
  parseFilenameLocalTimestamp,
  parseMetadataLocalTimestamp,
} from './local-timestamp'

export function detectionSeconds(event: TeslaCamEvent): number | null {
  const detected = parseMetadataLocalTimestamp(event.metadata?.timestamp ?? '')
  if (detected === null) return null
  const finalSegmentIndex = event.segments.length - 1
  for (const [index, segment] of event.segments.entries()) {
    const start = parseFilenameLocalTimestamp(segment.timestamp)
    if (start === null) continue
    const offset = detected.ordinalSeconds - start.ordinalSeconds
    const inRange = index === finalSegmentIndex
      ? offset >= 0 && offset <= 60
      : offset >= 0 && offset < 60
    if (inRange) return index * 60 + offset
  }
  return null
}

export function sentryInitialElapsedSeconds(event: TeslaCamEvent): number {
  if (event.category !== 'sentry') return 0
  const detected = detectionSeconds(event)
  return detected === null ? 0 : Math.max(0, detected - 15)
}

export function sentryPlaybackStartLabel(event: TeslaCamEvent): string | null {
  if (event.category !== 'sentry') return null
  const elapsedSeconds = sentryInitialElapsedSeconds(event)
  const segmentIndex = Math.floor(elapsedSeconds / 60)
  const segment = event.segments.at(segmentIndex)
  if (segment === undefined) return null
  return formatFilenameTimestampAtOffset(segment.timestamp, elapsedSeconds - segmentIndex * 60)
}
