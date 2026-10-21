import { cameraIds, type CameraId, type ParsedClipName } from './types'

function isCameraId(value: string): value is CameraId {
  return cameraIds.some((cameraId) => cameraId === value)
}

function daysInMonth(year: number, month: number): number {
  switch (month) {
    case 2:
      return year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0)
        ? 29
        : 28
    case 4:
    case 6:
    case 9:
    case 11:
      return 30
    default:
      return 31
  }
}

function isValidTimestamp(timestamp: string): boolean {
  const parts = /^(\d{4})-(\d{2})-(\d{2})_(\d{2})-(\d{2})-(\d{2})$/u.exec(
    timestamp,
  )
  if (parts === null) return false

  const values = parts.slice(1).map(Number)
  const [year, month, day, hour, minute, second] = values
  if (
    year === undefined ||
    month === undefined ||
    day === undefined ||
    hour === undefined ||
    minute === undefined ||
    second === undefined
  ) {
    return false
  }

  return (
    year > 0 &&
    month >= 1 &&
    month <= 12 &&
    day >= 1 &&
    day <= daysInMonth(year, month) &&
    hour <= 23 &&
    minute <= 59 &&
    second <= 59
  )
}

export function parseClipFileName(name: string): ParsedClipName | null {
  const match = /^(\d{4}-\d{2}-\d{2}_\d{2}-\d{2}-\d{2})-([a-z_]+)\.mp4$/u.exec(
    name,
  )
  if (match === null) return null

  const timestamp = match[1]
  const camera = match[2]
  if (
    timestamp === undefined ||
    camera === undefined ||
    !isValidTimestamp(timestamp) ||
    !isCameraId(camera)
  ) {
    return null
  }

  return { timestamp, camera }
}
