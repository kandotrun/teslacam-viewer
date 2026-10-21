import type { CameraId } from '../../teslacam/types'

export type StreamLifecycle = 'loading' | 'ready' | 'terminal'

export type PlayAttempt = {
  readonly id: number
  readonly element: HTMLVideoElement
  readonly intentGeneration: number
  readonly sourceGeneration: number
  readonly streamGeneration: number
  readonly status: 'pending' | 'fulfilled' | 'rejected'
}

export type PlaybackResolution = 'paused' | 'playing' | 'waiting' | 'failed'

export type PlaybackVideoEvent = 'loading' | 'ready' | 'unavailable' | 'timeupdate' | 'ended'

export type SynchronizedPlayback = {
  readonly playing: boolean
  readonly elapsedSeconds: number
  readonly durationSeconds: number
  readonly segmentIndex: number
  readonly segmentOffsetSeconds: number
  readonly rate: number
  readonly message: string | null
  readonly play: () => Promise<void>
  readonly pause: () => void
  readonly seek: (elapsedSeconds: number) => void
  readonly skip: (seconds: number) => void
  readonly setRate: (rate: number) => void
  readonly registerVideo: (cameraId: CameraId, element: HTMLVideoElement) => void
  readonly unregisterVideo: (cameraId: CameraId, element: HTMLVideoElement) => void
  readonly handleVideoEvent: (cameraId: CameraId, element: HTMLVideoElement, event: PlaybackVideoEvent) => void
}

export function resolvePlayback(
  intended: boolean,
  registeredCameras: readonly CameraId[],
  lifecycles: ReadonlyMap<CameraId, StreamLifecycle>,
  attempts: ReadonlyMap<CameraId, PlayAttempt>,
  successfulCameras: ReadonlySet<CameraId>,
): PlaybackResolution {
  if (!intended) return 'paused'
  if (successfulCameras.size > 0) return 'playing'
  const pending = registeredCameras.some((cameraId) => attempts.get(cameraId)?.status === 'pending')
  const loading = registeredCameras.some((cameraId) => lifecycles.get(cameraId) === 'loading')
  return pending || loading || registeredCameras.length === 0 ? 'waiting' : 'failed'
}

export function attemptMatches(
  attempt: PlayAttempt,
  current: PlayAttempt | undefined,
  currentElement: HTMLVideoElement | undefined,
  currentIntentGeneration: number,
  currentSourceGeneration: number,
  currentStreamGeneration: number,
): boolean {
  return current?.id === attempt.id &&
    currentElement === attempt.element &&
    currentIntentGeneration === attempt.intentGeneration &&
    currentSourceGeneration === attempt.sourceGeneration &&
    currentStreamGeneration === attempt.streamGeneration
}
