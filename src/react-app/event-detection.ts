import type { TeslaCamEvent } from '../teslacam/types'

function utcTimestamp(parts: readonly string[]): number | null {
  const values = parts.map(Number)
  const year = values[0]
  const month = values[1]
  const day = values[2]
  const hour = values[3]
  const minute = values[4]
  const second = values[5]
  if (
    year === undefined ||
    month === undefined ||
    day === undefined ||
    hour === undefined ||
    minute === undefined ||
    second === undefined
  ) return null
  const timestamp = Date.UTC(year, month - 1, day, hour, minute, second)
  const parsed = new Date(timestamp)
  return parsed.getUTCFullYear() === year &&
    parsed.getUTCMonth() === month - 1 &&
    parsed.getUTCDate() === day &&
    parsed.getUTCHours() === hour &&
    parsed.getUTCMinutes() === minute &&
    parsed.getUTCSeconds() === second
    ? timestamp
    : null
}

export function detectionSeconds(event: TeslaCamEvent, duration: number): number | null {
  const metadataTimestamp = event.metadata?.timestamp
  const startMatch = /^(\d{4})-(\d{2})-(\d{2})_(\d{2})-(\d{2})-(\d{2})$/u.exec(event.timestamp)
  if (metadataTimestamp === undefined || startMatch === null) return null
  const metadataMatch = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d{1,3})?(?:Z)?$/u.exec(metadataTimestamp)
  if (metadataMatch === null) return null
  const start = utcTimestamp(startMatch.slice(1))
  const detected = utcTimestamp(metadataMatch.slice(1))
  if (start === null || detected === null) return null
  const seconds = (detected - start) / 1000
  return seconds >= 0 && seconds <= duration ? seconds : null
}
