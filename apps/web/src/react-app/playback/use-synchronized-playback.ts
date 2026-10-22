import { useCallback, useEffect, useRef, useState } from 'react'

import { cameraIds, type CameraId, type TeslaCamEvent } from '../../teslacam/types'
import { createFrameSyncLoop, type FrameSyncLoop } from './frame-sync-loop'
import { createPlaybackAttemptCoordinator, type PlaybackAttemptCoordinator } from './playback-attempt-coordinator'
import type { PlaybackResolution, PlaybackVideoEvent, SynchronizedPlayback } from './playback-attempt-state'
import {
  createPlaybackRuntime,
  setPlaybackPosition,
  setPlaybackRate,
  setNoPlayableRequest,
  type PlaybackRuntime,
} from './playback-runtime'
import { electPlaybackSource } from './playback-source-election'
import { eventDuration, locatePlaybackTime } from './timeline'

export type { PlaybackVideoEvent, SynchronizedPlayback } from './playback-attempt-state'

const noPlayableMessage = '再生できるカメラ映像がありません。\n読み込み後に再試行してください。'
const rejectedMessage = '映像を再生できませんでした。\nブラウザの再生許可を確認して再試行できます。'

/** A seek to where the video already is would still stall it (a preloaded segment waits at 0). */
const samePositionSeconds = 0.001

function setMediaTime(element: HTMLVideoElement, seconds: number): void {
  if (Math.abs(element.currentTime - seconds) < samePositionSeconds) return
  element.currentTime = seconds
}

export function useSynchronizedPlayback(
  event: TeslaCamEvent,
  preferredCamera: CameraId | null = null,
  initialElapsedSeconds = 0,
): SynchronizedPlayback {
  const durationSeconds = eventDuration(event)
  const initialLocation = locatePlaybackTime(event, initialElapsedSeconds)
  const initialSeconds = initialLocation.segmentIndex * 60 + initialLocation.offsetSeconds
  const [elapsedSeconds, setElapsedSeconds] = useState(() => initialSeconds)
  const [playing, setPlaying] = useState(false)
  const [rate, setRateState] = useState(1)
  const [message, setMessage] = useState<string | null>(null)
  const [runtime] = useState<PlaybackRuntime>(() => (
    createPlaybackRuntime(initialSeconds, initialLocation.segmentIndex, true)
  ))
  // Read through refs so that every media callback keeps its identity: changing the expanded camera must
  // never re-run the camera windows' source effects (which would reload all six videos).
  const preferredCameraRef = useRef(preferredCamera)
  const updatePlaybackState = useCallback((resolution: PlaybackResolution): void => {
    setPlaying(resolution === 'playing')
    if (resolution === 'playing') {
      setNoPlayableRequest(runtime, false)
      setMessage(null)
    }
    if (resolution === 'failed') setMessage(runtime.noPlayableRequest ? noPlayableMessage : rejectedMessage)
  }, [runtime])
  const [coordinator] = useState<PlaybackAttemptCoordinator>(() => (
    createPlaybackAttemptCoordinator(runtime, updatePlaybackState)
  ))
  const [frameSync] = useState<FrameSyncLoop>(() => createFrameSyncLoop(runtime))

  const electSource = useCallback((): void => {
    electPlaybackSource(runtime, preferredCameraRef.current)
  }, [runtime])

  useEffect(() => {
    preferredCameraRef.current = preferredCamera
    electPlaybackSource(runtime, preferredCamera)
  }, [preferredCamera, runtime])

  const seek = useCallback((absoluteSeconds: number): void => {
    const location = locatePlaybackTime(event, absoluteSeconds)
    const segmentChanged = location.segmentIndex !== runtime.segmentIndex
    if (segmentChanged) {
      for (const registration of runtime.registrations.values()) registration.element.pause()
      coordinator.restartForSourceChange()
      setPlaying(false)
    }
    const normalized = location.segmentIndex * 60 + location.offsetSeconds
    setPlaybackPosition(runtime, normalized, location.segmentIndex)
    setElapsedSeconds(normalized)
    // In another segment every camera gets that segment's video, positioned once it is ready; seeking
    // the old videos would only flash their first frame.
    if (!segmentChanged) {
      for (const registration of runtime.registrations.values()) setMediaTime(registration.element, location.offsetSeconds)
    }
    frameSync.reset()
  }, [coordinator, event, frameSync, runtime])

  const pause = useCallback((): void => {
    coordinator.stop()
    setPlaying(false)
    for (const registration of runtime.registrations.values()) registration.element.pause()
    frameSync.reset()
  }, [coordinator, frameSync, runtime])

  const play = useCallback(async (): Promise<void> => {
    coordinator.start()
    const playable = cameraIds.filter((cameraId) => runtime.readyCameras.has(cameraId))
    setNoPlayableRequest(runtime, playable.length === 0)
    setMessage(playable.length === 0 ? noPlayableMessage : null)
    await coordinator.run(playable)
  }, [coordinator, runtime])

  const skip = useCallback((seconds: number): void => seek(runtime.elapsedSeconds + seconds), [runtime, seek])

  const setRate = useCallback((nextRate: number): void => {
    setPlaybackRate(runtime, nextRate)
    setRateState(nextRate)
    frameSync.reset()
  }, [frameSync, runtime])

  const unregisterVideo = useCallback((cameraId: CameraId, element: HTMLVideoElement): void => {
    if (runtime.registrations.get(cameraId)?.element !== element) return
    frameSync.unwatch(element)
    coordinator.invalidate(cameraId)
    runtime.registrations.delete(cameraId)
    runtime.lifecycles.delete(cameraId)
    runtime.readyCameras.delete(cameraId)
    if (runtime.sourceCamera === cameraId) electSource()
    coordinator.refresh()
  }, [coordinator, electSource, frameSync, runtime])

  /**
   * Registers the camera's video, replacing its previous one in place (a preloaded next segment): the
   * camera counts as loading until the new video reports ready, so intended playback never looks failed.
   */
  const registerVideo = useCallback((cameraId: CameraId, element: HTMLVideoElement): void => {
    const existing = runtime.registrations.get(cameraId)
    if (existing?.element === element) return
    runtime.registrations.set(cameraId, { element })
    runtime.lifecycles.set(cameraId, 'loading')
    frameSync.watch(element)
    if (existing === undefined) return
    frameSync.unwatch(existing.element)
    coordinator.invalidate(cameraId)
    runtime.readyCameras.delete(cameraId)
    if (runtime.sourceCamera === cameraId) electSource()
    coordinator.refresh()
  }, [coordinator, electSource, frameSync, runtime])

  const handleVideoEvent = useCallback((cameraId: CameraId, element: HTMLVideoElement, mediaEvent: PlaybackVideoEvent): void => {
    if (runtime.registrations.get(cameraId)?.element !== element) return
    electSource()
    switch (mediaEvent) {
      case 'loading':
        coordinator.invalidate(cameraId)
        runtime.lifecycles.set(cameraId, 'loading')
        runtime.readyCameras.delete(cameraId)
        if (runtime.sourceCamera === cameraId) electSource()
        coordinator.refresh()
        return
      case 'ready': {
        const wasReady = runtime.lifecycles.get(cameraId) === 'ready'
        runtime.lifecycles.set(cameraId, 'ready')
        runtime.readyCameras.add(cameraId)
        electSource()
        if (!wasReady) {
          const location = locatePlaybackTime(event, runtime.elapsedSeconds)
          setMediaTime(element, location.offsetSeconds)
        }
        element.playbackRate = runtime.rate
        if (runtime.intended && (!wasReady || (!runtime.attempts.has(cameraId) && !runtime.successfulCameras.has(cameraId)))) {
          setNoPlayableRequest(runtime, false)
          void coordinator.run([cameraId])
        }
        return
      }
      case 'unavailable':
        coordinator.invalidate(cameraId)
        runtime.lifecycles.set(cameraId, 'terminal')
        runtime.readyCameras.delete(cameraId)
        if (runtime.sourceCamera === cameraId) electSource()
        coordinator.refresh()
        return
      case 'timeupdate': {
        // The transport follows the source a few times a second; the frame sync loop keeps the other
        // cameras on its frames.
        if (runtime.sourceCamera !== cameraId) return
        const currentLocation = locatePlaybackTime(event, runtime.elapsedSeconds)
        const absoluteSeconds = currentLocation.segmentIndex * 60 + element.currentTime
        const location = locatePlaybackTime(event, absoluteSeconds)
        const normalized = location.segmentIndex * 60 + location.offsetSeconds
        setPlaybackPosition(runtime, normalized, location.segmentIndex)
        setElapsedSeconds(normalized)
        return
      }
      case 'ended': {
        if (runtime.sourceCamera !== cameraId) return
        const currentLocation = locatePlaybackTime(event, runtime.elapsedSeconds)
        const nextBoundary = (currentLocation.segmentIndex + 1) * 60
        if (nextBoundary < durationSeconds) {
          seek(nextBoundary)
          return
        }
        seek(durationSeconds)
        pause()
        return
      }
    }
  }, [coordinator, durationSeconds, electSource, event, pause, runtime, seek])

  useEffect(() => () => {
    coordinator.stop()
    for (const registration of runtime.registrations.values()) {
      registration.element.pause()
      frameSync.unwatch(registration.element)
    }
    runtime.registrations.clear()
    runtime.lifecycles.clear()
    runtime.readyCameras.clear()
  }, [coordinator, frameSync, runtime])

  const location = locatePlaybackTime(event, elapsedSeconds)
  return {
    playing, elapsedSeconds, durationSeconds,
    segmentIndex: location.segmentIndex, segmentOffsetSeconds: location.offsetSeconds,
    rate, message, play, pause, seek, skip, setRate,
    registerVideo, unregisterVideo, handleVideoEvent,
  }
}
