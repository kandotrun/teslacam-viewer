import { AlertTriangle, ImageOff } from 'lucide-react'
import { useCallback, useEffect, useRef, useState } from 'react'

import type { CameraId, LocalFileRef } from '../../teslacam/types'
import type { ObjectUrlRegistry } from '../playback/object-url-registry'
import type { PlaybackVideoEvent } from '../playback/use-synchronized-playback'

export type CameraStatus = 'loading' | 'ready' | 'missing' | 'unavailable' | 'corrupt' | 'unsupported'

type CameraTileProps = {
  readonly cameraId: CameraId
  readonly label: string
  readonly fileRef: LocalFileRef | undefined
  readonly registry: ObjectUrlRegistry
  readonly expanded: boolean
  readonly onToggle: () => void
  readonly onStatus: (cameraId: CameraId, status: CameraStatus) => void
  readonly registerVideo: (cameraId: CameraId, element: HTMLVideoElement) => void
  readonly unregisterVideo: (cameraId: CameraId, element: HTMLVideoElement) => void
  readonly onPlaybackEvent: (cameraId: CameraId, element: HTMLVideoElement, event: PlaybackVideoEvent) => void
}

function statusLabel(status: CameraStatus): string {
  switch (status) {
    case 'loading':
      return '読み込み中'
    case 'ready':
      return '再生できます'
    case 'missing':
      return '映像がありません'
    case 'unavailable':
      return '映像を読み込めません'
    case 'corrupt':
      return '映像が破損しています'
    case 'unsupported':
      return '未対応の映像形式です'
  }
}

function mediaErrorStatus(code: number | undefined, replacing: boolean): CameraStatus | null {
  switch (code) {
    case MediaError.MEDIA_ERR_DECODE:
      return 'corrupt'
    case MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED:
      return 'unsupported'
    case MediaError.MEDIA_ERR_NETWORK:
    case undefined:
      return 'unavailable'
    case MediaError.MEDIA_ERR_ABORTED:
      return replacing ? null : 'unavailable'
    default:
      return 'unavailable'
  }
}

export function CameraTile({
  cameraId,
  label,
  fileRef,
  registry,
  expanded,
  onToggle,
  onStatus,
  registerVideo,
  unregisterVideo,
  onPlaybackEvent,
}: CameraTileProps) {
  const videoRef = useRef<HTMLVideoElement>(null)
  const replacingRef = useRef(false)
  const [status, setStatus] = useState<CameraStatus>(fileRef === undefined ? 'missing' : 'loading')
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
    let active = true
    let acquiredFile: File | null = null
    replacingRef.current = true
    const update = (nextStatus: CameraStatus): void => {
      if (!active) return
      reportStatus(nextStatus)
    }
    onPlaybackEvent(cameraId, element, 'loading')
    element.pause()
    element.removeAttribute('src')
    element.load()
    if (fileRef === undefined) {
      replacingRef.current = false
      update('missing')
      onPlaybackEvent(cameraId, element, 'unavailable')
    } else {
      update('loading')
      void fileRef.getFile().then(
        (file) => {
          if (!active) return
          acquiredFile = file
          element.src = registry.acquire(file)
          replacingRef.current = false
          element.load()
        },
        (error: unknown) => {
          if (error instanceof Error || error instanceof DOMException) {
            replacingRef.current = false
            update('unavailable')
            onPlaybackEvent(cameraId, element, 'unavailable')
            return
          }
          throw error
        },
      )
    }

    return () => {
      active = false
      replacingRef.current = true
      element.pause()
      element.removeAttribute('src')
      element.load()
      if (acquiredFile !== null) registry.release(acquiredFile)
    }
  }, [cameraId, fileRef, onPlaybackEvent, registry, reportStatus])

  const terminal = status !== 'loading' && status !== 'ready'
  return (
    <button
      aria-label={`${label}カメラを${expanded ? 'グリッドへ戻す' : '拡大'}`}
      aria-pressed={expanded}
      className="camera-tile"
      data-expanded={expanded}
      data-status={status}
      type="button"
      onClick={onToggle}
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
      <span className="camera-tile__label">{label}</span>
      {status !== 'ready' && (
        <span className="camera-tile__state">
          {terminal ? <ImageOff aria-hidden="true" size={20} /> : <AlertTriangle aria-hidden="true" size={18} />}
          {statusLabel(status)}
        </span>
      )}
    </button>
  )
}
