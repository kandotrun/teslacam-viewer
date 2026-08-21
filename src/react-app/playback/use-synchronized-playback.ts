import { useCallback, useEffect, useRef, useState } from 'react'

import { cameraIds, type CameraId, type TeslaCamEvent } from '../../teslacam/types'
import { eventDuration, locatePlaybackTime } from './timeline'

type RegisteredVideo = {
  readonly element: HTMLVideoElement
}

function setMediaTime(element: HTMLVideoElement, seconds: number): void {
  element.currentTime = seconds
}

export type PlaybackVideoEvent = 'ready' | 'unavailable' | 'timeupdate' | 'ended'

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

export function useSynchronizedPlayback(
  event: TeslaCamEvent,
  preferredCamera: CameraId | null = null,
): SynchronizedPlayback {
  const durationSeconds = eventDuration(event)
  const [elapsedSeconds, setElapsedSeconds] = useState(0)
  const [playing, setPlaying] = useState(false)
  const [rate, setRateState] = useState(1)
  const [message, setMessage] = useState<string | null>(null)
  const registrations = useRef(new Map<CameraId, RegisteredVideo>())
  const readyCameras = useRef(new Set<CameraId>())
  const sourceCamera = useRef<CameraId | null>(null)
  const elapsedRef = useRef(0)
  const playingRef = useRef(false)
  const rateRef = useRef(1)
  const segmentRef = useRef(0)
  const generation = useRef(0)
  const preferredRef = useRef(preferredCamera)

  const electSource = useCallback((): void => {
    const preferred = preferredRef.current
    if (preferred !== null && readyCameras.current.has(preferred)) {
      sourceCamera.current = preferred
      return
    }
    sourceCamera.current = cameraIds.find((cameraId) => readyCameras.current.has(cameraId)) ?? null
  }, [])

  const syncElements = useCallback((absoluteSeconds: number): void => {
    const location = locatePlaybackTime(event, absoluteSeconds)
    segmentRef.current = location.segmentIndex
    for (const registration of registrations.current.values()) {
      setMediaTime(registration.element, location.offsetSeconds)
      registration.element.playbackRate = rateRef.current
    }
  }, [event])

  const seek = useCallback((absoluteSeconds: number): void => {
    generation.current += 1
    const location = locatePlaybackTime(event, absoluteSeconds)
    const normalized = location.segmentIndex * 60 + location.offsetSeconds
    elapsedRef.current = normalized
    setElapsedSeconds(normalized)
    syncElements(normalized)
  }, [event, syncElements])

  const pause = useCallback((): void => {
    generation.current += 1
    playingRef.current = false
    setPlaying(false)
    for (const registration of registrations.current.values()) registration.element.pause()
  }, [])

  const play = useCallback(async (): Promise<void> => {
    const playGeneration = generation.current + 1
    generation.current = playGeneration
    setMessage(null)
    const calls = [...readyCameras.current]
      .map((cameraId) => registrations.current.get(cameraId)?.element)
      .filter((element) => element !== undefined)
      .map((element) => element.play())
    if (calls.length === 0) {
      playingRef.current = false
      setPlaying(false)
      setMessage('再生できるカメラ映像がありません。\n読み込み後に再試行してください。')
      return
    }
    const results = await Promise.allSettled(calls)
    if (generation.current !== playGeneration) return
    const succeeded = results.some((result) => result.status === 'fulfilled')
    playingRef.current = succeeded
    setPlaying(succeeded)
    if (!succeeded) {
      setMessage('映像を再生できませんでした。\nブラウザの再生許可を確認して再試行できます。')
    }
  }, [])

  const skip = useCallback((seconds: number): void => {
    seek(elapsedRef.current + seconds)
  }, [seek])

  const setRate = useCallback((nextRate: number): void => {
    generation.current += 1
    rateRef.current = nextRate
    setRateState(nextRate)
    for (const registration of registrations.current.values()) {
      registration.element.playbackRate = nextRate
    }
  }, [])

  const unregisterVideo = useCallback((cameraId: CameraId, element: HTMLVideoElement): void => {
    const registration = registrations.current.get(cameraId)
    if (registration === undefined || registration.element !== element) return
    registrations.current.delete(cameraId)
    readyCameras.current.delete(cameraId)
    if (sourceCamera.current === cameraId) electSource()
  }, [electSource])

  const registerVideo = useCallback((cameraId: CameraId, element: HTMLVideoElement): void => {
    const existing = registrations.current.get(cameraId)
    if (existing?.element === element) return
    if (existing !== undefined) unregisterVideo(cameraId, existing.element)

    registrations.current.set(cameraId, { element })
  }, [unregisterVideo])

  const handleVideoEvent = useCallback((cameraId: CameraId, element: HTMLVideoElement, mediaEvent: PlaybackVideoEvent): void => {
    if (registrations.current.get(cameraId)?.element !== element) return
    switch (mediaEvent) {
      case 'ready': {
        readyCameras.current.add(cameraId)
        electSource()
        const location = locatePlaybackTime(event, elapsedRef.current)
        setMediaTime(element, location.offsetSeconds)
        element.playbackRate = rateRef.current
        if (playingRef.current) void element.play()
        return
      }
      case 'unavailable':
        readyCameras.current.delete(cameraId)
        if (sourceCamera.current === cameraId) electSource()
        return
      case 'timeupdate': {
        if (sourceCamera.current !== cameraId) return
        const absoluteSeconds = segmentRef.current * 60 + element.currentTime
        const location = locatePlaybackTime(event, absoluteSeconds)
        const normalized = location.segmentIndex * 60 + location.offsetSeconds
        elapsedRef.current = normalized
        segmentRef.current = location.segmentIndex
        setElapsedSeconds(normalized)
        for (const [otherCameraId, registration] of registrations.current) {
          if (otherCameraId === cameraId || !readyCameras.current.has(otherCameraId)) continue
          if (Math.abs(registration.element.currentTime - location.offsetSeconds) > 0.25) {
            setMediaTime(registration.element, location.offsetSeconds)
          }
        }
        return
      }
      case 'ended': {
        if (sourceCamera.current !== cameraId) return
        const nextBoundary = (segmentRef.current + 1) * 60
        if (nextBoundary < durationSeconds) {
          seek(nextBoundary)
          return
        }
        seek(durationSeconds)
        pause()
        return
      }
    }
  }, [durationSeconds, electSource, event, pause, seek])

  useEffect(() => () => {
    generation.current += 1
    for (const registration of registrations.current.values()) {
      registration.element.pause()
    }
    registrations.current.clear()
    readyCameras.current.clear()
  }, [])

  const location = locatePlaybackTime(event, elapsedSeconds)

  useEffect(() => {
    preferredRef.current = preferredCamera
    segmentRef.current = location.segmentIndex
    electSource()
  }, [electSource, location.segmentIndex, preferredCamera])

  return {
    playing,
    elapsedSeconds,
    durationSeconds,
    segmentIndex: location.segmentIndex,
    segmentOffsetSeconds: location.offsetSeconds,
    rate,
    message,
    play,
    pause,
    seek,
    skip,
    setRate,
    registerVideo,
    unregisterVideo,
    handleVideoEvent,
  }
}
