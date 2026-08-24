import type { CameraId } from '../../teslacam/types'
import type { PlayAttempt, StreamLifecycle } from './playback-attempt-state'

export type RegisteredVideo = { readonly element: HTMLVideoElement }

export type PlaybackRuntime = {
  registrations: Map<CameraId, RegisteredVideo>
  lifecycles: Map<CameraId, StreamLifecycle>
  readyCameras: Set<CameraId>
  successfulCameras: Set<CameraId>
  attempts: Map<CameraId, PlayAttempt>
  streamGenerations: Map<CameraId, number>
  sourceCamera: CameraId | null
  elapsedSeconds: number
  rate: number
  segmentIndex: number
  intended: boolean
  intentGeneration: number
  sourceGeneration: number
  attemptId: number
}

export function createPlaybackRuntime(elapsedSeconds = 0, segmentIndex = 0): PlaybackRuntime {
  return {
    registrations: new Map(),
    lifecycles: new Map(),
    readyCameras: new Set(),
    successfulCameras: new Set(),
    attempts: new Map(),
    streamGenerations: new Map(),
    sourceCamera: null,
    elapsedSeconds,
    rate: 1,
    segmentIndex,
    intended: false,
    intentGeneration: 0,
    sourceGeneration: 0,
    attemptId: 0,
  }
}

export function setPlaybackPosition(runtime: PlaybackRuntime, elapsedSeconds: number, segmentIndex: number): void {
  runtime.elapsedSeconds = elapsedSeconds
  runtime.segmentIndex = segmentIndex
}

export function setPlaybackRate(runtime: PlaybackRuntime, rate: number): void {
  runtime.rate = rate
}

export function setPlaybackSegment(runtime: PlaybackRuntime, segmentIndex: number): void {
  runtime.segmentIndex = segmentIndex
}
