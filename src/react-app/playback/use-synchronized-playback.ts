import { useCallback, useEffect, useState } from 'react'

import { cameraIds, type CameraId, type TeslaCamEvent } from '../../teslacam/types'
import { createPlaybackAttemptCoordinator, type PlaybackAttemptCoordinator } from './playback-attempt-coordinator'
import type { PlaybackResolution, PlaybackVideoEvent, SynchronizedPlayback } from './playback-attempt-state'
import {
  createPlaybackRuntime,
  setPlaybackPosition,
  setPlaybackRate,
  setPlaybackSegment,
  type PlaybackRuntime,
} from './playback-runtime'
import { electPlaybackSource } from './playback-source-election'
import { eventDuration, locatePlaybackTime } from './timeline'

export type { PlaybackVideoEvent, SynchronizedPlayback } from './playback-attempt-state'

const noPlayableMessage = '再生できるカメラ映像がありません。\n読み込み後に再試行してください。'
const rejectedMessage = '映像を再生できませんでした。\nブラウザの再生許可を確認して再試行できます。'

function setMediaTime(element: HTMLVideoElement, seconds: number): void {
  element.currentTime = seconds
}

export function useSynchronizedPlayback(event: TeslaCamEvent, preferredCamera: CameraId | null = null): SynchronizedPlayback {
  const durationSeconds = eventDuration(event)
  const [elapsedSeconds, setElapsedSeconds] = useState(0)
  const [playing, setPlaying] = useState(false)
  const [rate, setRateState] = useState(1)
  const [message, setMessage] = useState<string | null>(null)
  const [runtime] = useState<PlaybackRuntime>(createPlaybackRuntime)
  const updatePlaybackState = useCallback((resolution: PlaybackResolution): void => {
    setPlaying(resolution === 'playing')
    if (resolution === 'playing') setMessage(null)
    if (resolution === 'failed') setMessage(rejectedMessage)
  }, [])
  const [coordinator] = useState<PlaybackAttemptCoordinator>(() => (
    createPlaybackAttemptCoordinator(runtime, updatePlaybackState)
  ))

  const electSource = useCallback((): void => {
    electPlaybackSource(runtime, preferredCamera)
  }, [preferredCamera, runtime])

  const syncElements = useCallback((absoluteSeconds: number): void => {
    const location = locatePlaybackTime(event, absoluteSeconds)
    setPlaybackSegment(runtime, location.segmentIndex)
    for (const registration of runtime.registrations.values()) {
      setMediaTime(registration.element, location.offsetSeconds)
      registration.element.playbackRate = runtime.rate
    }
  }, [event, runtime])

  const seek = useCallback((absoluteSeconds: number): void => {
    const location = locatePlaybackTime(event, absoluteSeconds)
    if (location.segmentIndex !== runtime.segmentIndex) {
      for (const registration of runtime.registrations.values()) registration.element.pause()
      coordinator.restartForSourceChange()
      setPlaying(false)
    }
    const normalized = location.segmentIndex * 60 + location.offsetSeconds
    setPlaybackPosition(runtime, normalized, location.segmentIndex)
    setElapsedSeconds(normalized)
    syncElements(normalized)
  }, [coordinator, event, runtime, syncElements])

  const pause = useCallback((): void => {
    coordinator.stop()
    setPlaying(false)
    for (const registration of runtime.registrations.values()) registration.element.pause()
  }, [coordinator, runtime])

  const play = useCallback(async (): Promise<void> => {
    coordinator.start()
    setMessage(null)
    const playable = cameraIds.filter((cameraId) => runtime.readyCameras.has(cameraId))
    if (playable.length === 0) setMessage(noPlayableMessage)
    await coordinator.run(playable)
  }, [coordinator, runtime])

  const skip = useCallback((seconds: number): void => seek(runtime.elapsedSeconds + seconds), [runtime, seek])

  const setRate = useCallback((nextRate: number): void => {
    setPlaybackRate(runtime, nextRate)
    setRateState(nextRate)
    for (const registration of runtime.registrations.values()) registration.element.playbackRate = nextRate
  }, [runtime])

  const unregisterVideo = useCallback((cameraId: CameraId, element: HTMLVideoElement): void => {
    if (runtime.registrations.get(cameraId)?.element !== element) return
    coordinator.invalidate(cameraId)
    runtime.registrations.delete(cameraId)
    runtime.lifecycles.delete(cameraId)
    runtime.readyCameras.delete(cameraId)
    if (runtime.sourceCamera === cameraId) electSource()
    coordinator.refresh()
  }, [coordinator, electSource, runtime])

  const registerVideo = useCallback((cameraId: CameraId, element: HTMLVideoElement): void => {
    const existing = runtime.registrations.get(cameraId)
    if (existing?.element === element) return
    if (existing !== undefined) unregisterVideo(cameraId, existing.element)
    runtime.registrations.set(cameraId, { element })
    runtime.lifecycles.set(cameraId, 'loading')
  }, [runtime, unregisterVideo])

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
        const location = locatePlaybackTime(event, runtime.elapsedSeconds)
        setMediaTime(element, location.offsetSeconds)
        element.playbackRate = runtime.rate
        if (runtime.intended && (!wasReady || (!runtime.attempts.has(cameraId) && !runtime.successfulCameras.has(cameraId)))) {
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
        if (runtime.sourceCamera !== cameraId) return
        const currentLocation = locatePlaybackTime(event, runtime.elapsedSeconds)
        const absoluteSeconds = currentLocation.segmentIndex * 60 + element.currentTime
        const location = locatePlaybackTime(event, absoluteSeconds)
        const normalized = location.segmentIndex * 60 + location.offsetSeconds
        setPlaybackPosition(runtime, normalized, location.segmentIndex)
        setElapsedSeconds(normalized)
        for (const [otherCameraId, registration] of runtime.registrations) {
          if (otherCameraId === cameraId || !runtime.readyCameras.has(otherCameraId)) continue
          if (Math.abs(registration.element.currentTime - location.offsetSeconds) > 0.25) {
            setMediaTime(registration.element, location.offsetSeconds)
          }
        }
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
    for (const registration of runtime.registrations.values()) registration.element.pause()
    runtime.registrations.clear()
    runtime.lifecycles.clear()
    runtime.readyCameras.clear()
  }, [coordinator, runtime])

  const location = locatePlaybackTime(event, elapsedSeconds)
  return {
    playing, elapsedSeconds, durationSeconds,
    segmentIndex: location.segmentIndex, segmentOffsetSeconds: location.offsetSeconds,
    rate, message, play, pause, seek, skip, setRate,
    registerVideo, unregisterVideo, handleVideoEvent,
  }
}
