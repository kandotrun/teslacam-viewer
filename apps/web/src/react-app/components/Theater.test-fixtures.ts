import { vi } from 'vitest'

import { cameraIds, type CameraId, type LocalFileRef, type TeslaCamEvent } from '../../teslacam/types'

export const labels = ['フロント', 'リア', '左ピラー', '左リピーター', '右ピラー', '右リピーター'] as const

export function fileRef(name: string, getFile = async () => new File(['clip'], name, { type: 'video/mp4' })): LocalFileRef {
  return { name, relativePath: `SavedClips/event/${name}`, size: 4, type: 'video/mp4', getFile }
}

export function eventWithCameras(cameras: readonly CameraId[] = cameraIds, id = 'event'): TeslaCamEvent {
  const refs: Partial<Record<CameraId, LocalFileRef>> = {}
  for (const cameraId of cameras) refs[cameraId] = fileRef(`${cameraId}.mp4`)
  return {
    id,
    category: 'saved',
    timestamp: '2026-08-21_12-00-00',
    metadata: { timestamp: '2026-08-21T12:00:30' },
    segments: [{ timestamp: '2026-08-21_12-00-00', cameras: refs }],
  }
}

export function firstCameraVideo(elements: readonly HTMLElement[]): HTMLVideoElement {
  const video = elements.find((element): element is HTMLVideoElement => element instanceof HTMLVideoElement)
  if (video === undefined) throw new Error('first camera video is missing')
  return video
}

export function sentryEventWithCameras(id: string): TeslaCamEvent {
  const baseEvent = eventWithCameras()
  const firstSegment = baseEvent.segments.at(0)
  if (firstSegment === undefined) throw new Error('camera event is missing its first segment')
  return {
    ...baseEvent,
    id,
    category: 'sentry',
    metadata: { timestamp: '2026-08-21T12:01:20' },
    segments: [firstSegment, { ...firstSegment, timestamp: '2026-08-21_12-01-00' }],
  }
}

export function setupTheaterMedia(): void {
  vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:camera')
  vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => undefined)
  HTMLMediaElement.prototype.load = vi.fn()
  HTMLMediaElement.prototype.pause = vi.fn()
  HTMLMediaElement.prototype.play = vi.fn(async () => undefined)
}
