import { LoaderCircle, VideoOff } from 'lucide-react'
import { memo, useCallback, useEffect, useId, useLayoutEffect, useRef, useState } from 'react'

import type { CameraId, LocalFileRef } from '../../../teslacam/types'
import type { ObjectUrlRegistry } from '../../playback/object-url-registry'
import type { BufferSlot } from '../../playback/segment-buffers'
import type { PlaybackVideoEvent } from '../../playback/use-synchronized-playback'
import { cameraStatusLabel, mediaErrorStatus, terminalStatuses, type CameraStatus } from '../../presentation/camera-status'

type PlaybackBindings = {
  readonly registerVideo: (cameraId: CameraId, element: HTMLVideoElement) => void
  readonly unregisterVideo: (cameraId: CameraId, element: HTMLVideoElement) => void
  readonly onPlaybackEvent: (cameraId: CameraId, element: HTMLVideoElement, event: PlaybackVideoEvent) => void
}

type CameraWindowProps = PlaybackBindings & {
  readonly cameraId: CameraId
  readonly label: string
  /** The camera's file in the playing segment. */
  readonly fileRef: LocalFileRef | undefined
  /** The camera's file in the segment the spare video holds (the next one, preloaded). */
  readonly spareFileRef: LocalFileRef | undefined
  /** Which of the two videos plays `fileRef` (see `segmentBuffers`). */
  readonly activeSlot: BufferSlot
  readonly registry: ObjectUrlRegistry
  /** Grid area on the stage; `hidden` keeps the window mounted but not displayed. */
  readonly area: string
  readonly active: boolean
  readonly expanded: boolean
  readonly revealIndex: number
  readonly onToggle: (cameraId: CameraId) => void
  readonly onStatus: (cameraId: CameraId, status: CameraStatus) => void
}

type SlotStatuses = readonly [CameraStatus, CameraStatus]

const initialStatus = (fileRef: LocalFileRef | undefined): CameraStatus => (fileRef === undefined ? 'missing' : 'loading')

/**
 * One camera window. Its two `<video>` elements live as long as the viewer session: changing the layout
 * only changes `area`, so layouts never reload media. One video plays the current segment while the
 * other, hidden, preloads the next; at the boundary they swap roles and nothing loads.
 */
export const CameraWindow = memo(function CameraWindow({
  cameraId,
  label,
  fileRef,
  spareFileRef,
  activeSlot,
  registry,
  area,
  active,
  expanded,
  revealIndex,
  onToggle,
  onStatus,
  registerVideo,
  unregisterVideo,
  onPlaybackEvent,
}: CameraWindowProps) {
  const slotFiles = activeSlot === 0 ? [fileRef, spareFileRef] as const : [spareFileRef, fileRef] as const
  const [statuses, setStatuses] = useState<SlotStatuses>(() => [initialStatus(slotFiles[0]), initialStatus(slotFiles[1])])
  const status = statuses[activeSlot]
  const stateTextId = useId()
  // Read by the videos' callbacks, which keep their identity so a swap never reloads either video.
  const activeSlotRef = useRef(activeSlot)

  useLayoutEffect(() => {
    activeSlotRef.current = activeSlot
  }, [activeSlot])

  // After a swap the window reports the status of the video that took over.
  useEffect(() => {
    onStatus(cameraId, status)
  }, [cameraId, onStatus, status])

  const reportSlotStatus = useCallback((slot: BufferSlot, nextStatus: CameraStatus): void => {
    setStatuses((current) => {
      if (current[slot] === nextStatus) return current
      return slot === 0 ? [nextStatus, current[1]] : [current[0], nextStatus]
    })
    if (slot === activeSlotRef.current) onStatus(cameraId, nextStatus)
  }, [cameraId, onStatus])

  return (
    <div className="camera-slot" data-area={area} data-camera={cameraId} style={area === 'hidden' ? undefined : { gridArea: area }}>
      <button
        aria-describedby={status === 'ready' ? undefined : stateTextId}
        aria-label={`${label}カメラを${expanded ? 'グリッドへ戻す' : '拡大'}`}
        aria-pressed={expanded}
        className="camera-window"
        data-active={active}
        data-camera={cameraId}
        data-expanded={expanded}
        data-reveal-index={revealIndex}
        data-status={status}
        type="button"
        onClick={() => onToggle(cameraId)}
      >
        {([0, 1] as const).map((slot) => (slot === activeSlot || slotFiles[slot] !== undefined) && (
          <CameraVideo
            active={slot === activeSlot}
            cameraId={cameraId}
            fileRef={slotFiles[slot]}
            key={slot}
            registerVideo={registerVideo}
            registry={registry}
            slot={slot}
            unregisterVideo={unregisterVideo}
            onPlaybackEvent={onPlaybackEvent}
            onSlotStatus={reportSlotStatus}
          />
        ))}
        {status !== 'ready' && (
          <span className="camera-window__state">
            {status === 'loading'
              ? <LoaderCircle aria-hidden="true" className="spinner" size={20} />
              : <VideoOff aria-hidden="true" size={22} />}
            <span className="camera-window__state-text" id={stateTextId}>{cameraStatusLabel(status)}</span>
          </span>
        )}
        <span className="camera-window__label">{label}</span>
      </button>
    </div>
  )
})

type CameraVideoProps = PlaybackBindings & {
  readonly cameraId: CameraId
  readonly slot: BufferSlot
  readonly fileRef: LocalFileRef | undefined
  /** Plays for the camera; otherwise hidden, holding (preloading) its file. */
  readonly active: boolean
  readonly registry: ObjectUrlRegistry
  readonly onSlotStatus: (slot: BufferSlot, status: CameraStatus) => void
}

/** One of a window's two videos. While active it is the camera's video in synchronized playback. */
function CameraVideo({
  cameraId,
  slot,
  fileRef,
  active,
  registry,
  onSlotStatus,
  registerVideo,
  unregisterVideo,
  onPlaybackEvent,
}: CameraVideoProps) {
  const videoRef = useRef<HTMLVideoElement>(null)
  const replacingRef = useRef(false)
  const statusRef = useRef<CameraStatus>(initialStatus(fileRef))
  const reportStatus = useCallback((nextStatus: CameraStatus): void => {
    statusRef.current = nextStatus
    onSlotStatus(slot, nextStatus)
  }, [onSlotStatus, slot])

  // Taking over a camera replaces its previous video in place; a preloaded video is already ready.
  useEffect(() => {
    const element = videoRef.current
    if (element === null || !active) return
    registerVideo(cameraId, element)
    if (terminalStatuses.includes(statusRef.current)) onPlaybackEvent(cameraId, element, 'unavailable')
    else if (element.readyState >= HTMLMediaElement.HAVE_FUTURE_DATA) onPlaybackEvent(cameraId, element, 'ready')
  }, [active, cameraId, onPlaybackEvent, registerVideo])

  useEffect(() => {
    const element = videoRef.current
    if (element === null) return undefined
    return () => unregisterVideo(cameraId, element)
  }, [cameraId, unregisterVideo])

  useEffect(() => {
    const element = videoRef.current
    if (element === null) return undefined
    let current = true
    let acquiredFile: File | null = null
    replacingRef.current = true
    onPlaybackEvent(cameraId, element, 'loading')
    element.pause()
    element.removeAttribute('src')
    element.load()
    if (fileRef === undefined) {
      replacingRef.current = false
      reportStatus('missing')
      onPlaybackEvent(cameraId, element, 'unavailable')
    } else {
      reportStatus('loading')
      void fileRef.getFile().then(
        (file) => {
          if (!current) return
          acquiredFile = file
          element.src = registry.acquire(file)
          replacingRef.current = false
          element.load()
        },
        () => {
          if (!current) return
          replacingRef.current = false
          reportStatus('unavailable')
          onPlaybackEvent(cameraId, element, 'unavailable')
        },
      )
    }
    return () => {
      current = false
      replacingRef.current = true
      statusRef.current = 'loading'
      element.pause()
      element.removeAttribute('src')
      element.load()
      if (acquiredFile !== null) registry.release(acquiredFile)
    }
  }, [cameraId, fileRef, onPlaybackEvent, registry, reportStatus])

  return (
    <video
      data-buffer={active ? 'active' : 'spare'}
      data-testid={active ? 'camera-video' : undefined}
      muted
      playsInline
      preload={active ? 'metadata' : 'auto'}
      ref={videoRef}
      onCanPlay={(mediaEvent) => {
        reportStatus('ready')
        onPlaybackEvent(cameraId, mediaEvent.currentTarget, 'ready')
      }}
      onEnded={(mediaEvent) => onPlaybackEvent(cameraId, mediaEvent.currentTarget, 'ended')}
      onError={(mediaEvent) => {
        const errorStatus = mediaErrorStatus(mediaEvent.currentTarget.error?.code, replacingRef.current)
        if (errorStatus === null) return
        reportStatus(errorStatus)
        onPlaybackEvent(cameraId, mediaEvent.currentTarget, 'unavailable')
      }}
      onTimeUpdate={(mediaEvent) => onPlaybackEvent(cameraId, mediaEvent.currentTarget, 'timeupdate')}
    />
  )
}
