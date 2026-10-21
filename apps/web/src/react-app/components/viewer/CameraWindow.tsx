import { LoaderCircle, VideoOff } from 'lucide-react'
import { memo, useCallback, useEffect, useId, useRef, useState } from 'react'

import type { CameraId, LocalFileRef } from '../../../teslacam/types'
import type { ObjectUrlRegistry } from '../../playback/object-url-registry'
import type { PlaybackVideoEvent } from '../../playback/use-synchronized-playback'
import { cameraStatusLabel, mediaErrorStatus, type CameraStatus } from '../../presentation/camera-status'

type CameraWindowProps = {
  readonly cameraId: CameraId
  readonly label: string
  readonly fileRef: LocalFileRef | undefined
  readonly registry: ObjectUrlRegistry
  /** Grid area on the stage; `hidden` keeps the window mounted but not displayed. */
  readonly area: string
  readonly active: boolean
  readonly expanded: boolean
  readonly revealIndex: number
  readonly onToggle: (cameraId: CameraId) => void
  readonly onStatus: (cameraId: CameraId, status: CameraStatus) => void
  readonly registerVideo: (cameraId: CameraId, element: HTMLVideoElement) => void
  readonly unregisterVideo: (cameraId: CameraId, element: HTMLVideoElement) => void
  readonly onPlaybackEvent: (cameraId: CameraId, element: HTMLVideoElement, event: PlaybackVideoEvent) => void
}

/**
 * One camera window. The `<video>` element lives as long as the viewer session: changing the layout only
 * changes `area`, and the source effect depends on the file alone, so layouts never reload media.
 */
export const CameraWindow = memo(function CameraWindow({
  cameraId,
  label,
  fileRef,
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
  const videoRef = useRef<HTMLVideoElement>(null)
  const replacingRef = useRef(false)
  const [status, setStatus] = useState<CameraStatus>(fileRef === undefined ? 'missing' : 'loading')
  const stateTextId = useId()
  const reportStatus = useCallback((nextStatus: CameraStatus): void => {
    setStatus(nextStatus)
    onStatus(cameraId, nextStatus)
  }, [cameraId, onStatus])

  useEffect(() => {
    const element = videoRef.current
    if (element === null) return undefined
    registerVideo(cameraId, element)
    return () => unregisterVideo(cameraId, element)
  }, [cameraId, registerVideo, unregisterVideo])

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
      element.pause()
      element.removeAttribute('src')
      element.load()
      if (acquiredFile !== null) registry.release(acquiredFile)
    }
  }, [cameraId, fileRef, onPlaybackEvent, registry, reportStatus])

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
        <video
          data-testid="camera-video"
          muted
          playsInline
          preload="metadata"
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
