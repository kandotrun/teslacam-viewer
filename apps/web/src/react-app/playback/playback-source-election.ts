import { cameraIds, type CameraId } from '../../teslacam/types'
import type { PlaybackRuntime } from './playback-runtime'

export function electPlaybackSource(runtime: PlaybackRuntime, preferredCamera: CameraId | null): void {
  if (preferredCamera !== null && runtime.readyCameras.has(preferredCamera)) {
    runtime.sourceCamera = preferredCamera
    return
  }
  runtime.sourceCamera = cameraIds.find((cameraId) => runtime.readyCameras.has(cameraId)) ?? null
}
