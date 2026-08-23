import { cameraIds, type CameraId, type TeslaCamEvent } from '../teslacam/types'
import { eventDuration } from './playback/timeline'

export const cameraLabels: Readonly<Record<CameraId, string>> = {
  front: 'フロント',
  back: 'リア',
  left_pillar: '左ピラー',
  left_repeater: '左リピーター',
  right_pillar: '右ピラー',
  right_repeater: '右リピーター',
}

export type CameraAvailability = {
  readonly available: number
  readonly total: number
  readonly missing: readonly CameraId[]
}

export function cameraAvailability(event: TeslaCamEvent): CameraAvailability {
  const missing = cameraIds.filter((cameraId) =>
    event.segments.length === 0 ||
    event.segments.some((segment) => segment.cameras[cameraId] === undefined),
  )
  return {
    available: cameraIds.length - missing.length,
    total: cameraIds.length,
    missing,
  }
}

export function cameraAvailabilityLabel(availability: CameraAvailability): string {
  return `${availability.available}/${availability.total} カメラ`
}

export function missingCameraLabel(availability: CameraAvailability): string | null {
  if (availability.missing.length === 0) return null
  return `${availability.missing.map((cameraId) => cameraLabels[cameraId]).join('、')}なし`
}

export function formatDuration(seconds: number): string {
  if (seconds < 60) return `${seconds}秒`
  const hours = Math.floor(seconds / 3600)
  const minutes = Math.floor(seconds % 3600 / 60)
  if (hours === 0) return `${minutes}分`
  if (minutes === 0) return `${hours}時間`
  return `${hours}時間${minutes}分`
}

export function eventDurationLabel(event: TeslaCamEvent): string {
  return formatDuration(eventDuration(event))
}

export function segmentCountLabel(event: TeslaCamEvent): string {
  return `${event.segments.length}セグメント`
}
