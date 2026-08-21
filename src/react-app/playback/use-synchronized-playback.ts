import { useCallback, useEffect, useRef, useState } from 'react'

import { cameraIds, type CameraId, type TeslaCamEvent } from '../../teslacam/types'
import {
  attemptMatches,
  resolvePlayback,
  type PlayAttempt,
  type PlaybackVideoEvent,
  type SynchronizedPlayback,
  type StreamLifecycle,
} from './playback-attempt-state'
import { eventDuration, locatePlaybackTime } from './timeline'

export type { PlaybackVideoEvent, SynchronizedPlayback } from './playback-attempt-state'

type RegisteredVideo = { readonly element: HTMLVideoElement }

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
  const registrations = useRef(new Map<CameraId, RegisteredVideo>())
  const lifecycles = useRef(new Map<CameraId, StreamLifecycle>())
  const readyCameras = useRef(new Set<CameraId>())
  const successfulCameras = useRef(new Set<CameraId>())
  const attempts = useRef(new Map<CameraId, PlayAttempt>())
  const streamGenerations = useRef(new Map<CameraId, number>())
  const sourceCamera = useRef<CameraId | null>(null)
  const elapsedRef = useRef(0)
  const rateRef = useRef(1)
  const segmentRef = useRef(0)
  const intentRef = useRef(false)
  const intentGeneration = useRef(0)
  const sourceGeneration = useRef(0)
  const attemptId = useRef(0)
  const preferredRef = useRef(preferredCamera)

  const electSource = useCallback((): void => {
    const preferred = preferredRef.current
    if (preferred !== null && readyCameras.current.has(preferred)) {
      sourceCamera.current = preferred
      return
    }
    sourceCamera.current = cameraIds.find((cameraId) => readyCameras.current.has(cameraId)) ?? null
  }, [])

  const refreshPlaybackState = useCallback((): void => {
    const registeredCameras = [...registrations.current.keys()]
    const resolution = resolvePlayback(
      intentRef.current,
      registeredCameras,
      lifecycles.current,
      attempts.current,
      successfulCameras.current,
    )
    setPlaying(resolution === 'playing')
    if (resolution === 'playing') setMessage(null)
    if (resolution === 'failed') setMessage(rejectedMessage)
  }, [])

  const invalidateCamera = useCallback((cameraId: CameraId): void => {
    streamGenerations.current.set(cameraId, (streamGenerations.current.get(cameraId) ?? 0) + 1)
    attempts.current.delete(cameraId)
    successfulCameras.current.delete(cameraId)
  }, [])

  const runPlayAttempts = useCallback(async (cameraIdsToPlay: readonly CameraId[]): Promise<void> => {
    const calls: Array<{ readonly cameraId: CameraId; readonly attempt: PlayAttempt; readonly promise: Promise<void> }> = []
    for (const cameraId of cameraIdsToPlay) {
      const registration = registrations.current.get(cameraId)
      if (registration === undefined || lifecycles.current.get(cameraId) !== 'ready') continue
      const attempt: PlayAttempt = {
        id: attemptId.current + 1,
        element: registration.element,
        intentGeneration: intentGeneration.current,
        sourceGeneration: sourceGeneration.current,
        streamGeneration: streamGenerations.current.get(cameraId) ?? 0,
        status: 'pending',
      }
      attemptId.current = attempt.id
      attempts.current.set(cameraId, attempt)
      calls.push({ cameraId, attempt, promise: registration.element.play() })
    }
    if (calls.length === 0) {
      refreshPlaybackState()
      return
    }
    refreshPlaybackState()
    const results = await Promise.allSettled(calls.map((call) => call.promise))
    for (const [index, call] of calls.entries()) {
      const result = results[index]
      if (result === undefined || !attemptMatches(
        call.attempt,
        attempts.current.get(call.cameraId),
        registrations.current.get(call.cameraId)?.element,
        intentGeneration.current,
        sourceGeneration.current,
        streamGenerations.current.get(call.cameraId) ?? 0,
      )) continue
      attempts.current.set(call.cameraId, { ...call.attempt, status: result.status })
      if (result.status === 'fulfilled') successfulCameras.current.add(call.cameraId)
      else successfulCameras.current.delete(call.cameraId)
    }
    refreshPlaybackState()
  }, [refreshPlaybackState])

  const syncElements = useCallback((absoluteSeconds: number): void => {
    const location = locatePlaybackTime(event, absoluteSeconds)
    segmentRef.current = location.segmentIndex
    for (const registration of registrations.current.values()) {
      setMediaTime(registration.element, location.offsetSeconds)
      registration.element.playbackRate = rateRef.current
    }
  }, [event])

  const seek = useCallback((absoluteSeconds: number): void => {
    const location = locatePlaybackTime(event, absoluteSeconds)
    if (location.segmentIndex !== segmentRef.current) {
      sourceGeneration.current += 1
      attempts.current.clear()
      successfulCameras.current.clear()
      setPlaying(false)
    }
    const normalized = location.segmentIndex * 60 + location.offsetSeconds
    elapsedRef.current = normalized
    setElapsedSeconds(normalized)
    syncElements(normalized)
  }, [event, syncElements])

  const pause = useCallback((): void => {
    intentRef.current = false
    intentGeneration.current += 1
    attempts.current.clear()
    successfulCameras.current.clear()
    setPlaying(false)
    for (const registration of registrations.current.values()) registration.element.pause()
  }, [])

  const play = useCallback(async (): Promise<void> => {
    intentRef.current = true
    intentGeneration.current += 1
    attempts.current.clear()
    successfulCameras.current.clear()
    setMessage(null)
    const playable = cameraIds.filter((cameraId) => readyCameras.current.has(cameraId))
    if (playable.length === 0) setMessage(noPlayableMessage)
    await runPlayAttempts(playable)
  }, [runPlayAttempts])

  const skip = useCallback((seconds: number): void => seek(elapsedRef.current + seconds), [seek])

  const setRate = useCallback((nextRate: number): void => {
    rateRef.current = nextRate
    setRateState(nextRate)
    for (const registration of registrations.current.values()) registration.element.playbackRate = nextRate
  }, [])

  const unregisterVideo = useCallback((cameraId: CameraId, element: HTMLVideoElement): void => {
    if (registrations.current.get(cameraId)?.element !== element) return
    invalidateCamera(cameraId)
    registrations.current.delete(cameraId)
    lifecycles.current.delete(cameraId)
    readyCameras.current.delete(cameraId)
    if (sourceCamera.current === cameraId) electSource()
    refreshPlaybackState()
  }, [electSource, invalidateCamera, refreshPlaybackState])

  const registerVideo = useCallback((cameraId: CameraId, element: HTMLVideoElement): void => {
    const existing = registrations.current.get(cameraId)
    if (existing?.element === element) return
    if (existing !== undefined) unregisterVideo(cameraId, existing.element)
    registrations.current.set(cameraId, { element })
    lifecycles.current.set(cameraId, 'loading')
  }, [unregisterVideo])

  const handleVideoEvent = useCallback((cameraId: CameraId, element: HTMLVideoElement, mediaEvent: PlaybackVideoEvent): void => {
    if (registrations.current.get(cameraId)?.element !== element) return
    switch (mediaEvent) {
      case 'loading':
        invalidateCamera(cameraId)
        lifecycles.current.set(cameraId, 'loading')
        readyCameras.current.delete(cameraId)
        if (sourceCamera.current === cameraId) electSource()
        refreshPlaybackState()
        return
      case 'ready': {
        const wasReady = lifecycles.current.get(cameraId) === 'ready'
        lifecycles.current.set(cameraId, 'ready')
        readyCameras.current.add(cameraId)
        electSource()
        const location = locatePlaybackTime(event, elapsedRef.current)
        setMediaTime(element, location.offsetSeconds)
        element.playbackRate = rateRef.current
        if (intentRef.current && (!wasReady || (!attempts.current.has(cameraId) && !successfulCameras.current.has(cameraId)))) void runPlayAttempts([cameraId])
        return
      }
      case 'unavailable':
        invalidateCamera(cameraId)
        lifecycles.current.set(cameraId, 'terminal')
        readyCameras.current.delete(cameraId)
        if (sourceCamera.current === cameraId) electSource()
        refreshPlaybackState()
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
          if (Math.abs(registration.element.currentTime - location.offsetSeconds) > 0.25) setMediaTime(registration.element, location.offsetSeconds)
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
  }, [durationSeconds, electSource, event, invalidateCamera, pause, refreshPlaybackState, runPlayAttempts, seek])

  useEffect(() => () => {
    intentRef.current = false
    intentGeneration.current += 1
    for (const registration of registrations.current.values()) registration.element.pause()
    registrations.current.clear()
    lifecycles.current.clear()
    readyCameras.current.clear()
    attempts.current.clear()
    successfulCameras.current.clear()
  }, [])

  const location = locatePlaybackTime(event, elapsedSeconds)
  useEffect(() => {
    preferredRef.current = preferredCamera
    segmentRef.current = location.segmentIndex
    electSource()
  }, [electSource, location.segmentIndex, preferredCamera])

  return {
    playing, elapsedSeconds, durationSeconds,
    segmentIndex: location.segmentIndex, segmentOffsetSeconds: location.offsetSeconds,
    rate, message, play, pause, seek, skip, setRate,
    registerVideo, unregisterVideo, handleVideoEvent,
  }
}
