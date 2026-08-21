import { useCallback, useEffect, useMemo, useState } from 'react'

import { cameraIds, type CameraId, type TeslaCamEvent } from '../../teslacam/types'
import { ObjectUrlRegistry } from '../playback/object-url-registry'
import { useSynchronizedPlayback } from '../playback/use-synchronized-playback'
import { CameraTile, type CameraStatus } from './CameraTile'
import { PlaybackControls } from './PlaybackControls'

export type TheaterProps = {
  readonly event: TeslaCamEvent
  readonly onPrevious: () => void
  readonly onNext: () => void
  readonly hasPrevious: boolean
  readonly hasNext: boolean
}

const cameraLabels: Readonly<Record<CameraId, string>> = {
  front: 'フロント',
  back: 'リア',
  left_pillar: '左ピラー',
  left_repeater: '左リピーター',
  right_pillar: '右ピラー',
  right_repeater: '右リピーター',
}

const terminalStatuses: readonly CameraStatus[] = ['missing', 'unavailable', 'corrupt', 'unsupported']

function utcTimestamp(parts: readonly string[]): number | null {
  const values = parts.map(Number)
  const year = values[0]
  const month = values[1]
  const day = values[2]
  const hour = values[3]
  const minute = values[4]
  const second = values[5]
  if (year === undefined || month === undefined || day === undefined || hour === undefined || minute === undefined || second === undefined) return null
  const timestamp = Date.UTC(year, month - 1, day, hour, minute, second)
  const parsed = new Date(timestamp)
  return parsed.getUTCFullYear() === year &&
    parsed.getUTCMonth() === month - 1 &&
    parsed.getUTCDate() === day &&
    parsed.getUTCHours() === hour &&
    parsed.getUTCMinutes() === minute &&
    parsed.getUTCSeconds() === second
    ? timestamp
    : null
}

function detectionSeconds(event: TeslaCamEvent, duration: number): number | null {
  const metadataTimestamp = event.metadata?.timestamp
  const startMatch = /^(\d{4})-(\d{2})-(\d{2})_(\d{2})-(\d{2})-(\d{2})$/u.exec(event.timestamp)
  if (metadataTimestamp === undefined || startMatch === null) return null
  const metadataMatch = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})(?:\.\d{1,3})?(?:Z)?$/u.exec(metadataTimestamp)
  if (metadataMatch === null) return null
  const start = utcTimestamp(startMatch.slice(1))
  const detected = utcTimestamp(metadataMatch.slice(1))
  if (start === null || detected === null) return null
  const seconds = (detected - start) / 1000
  return seconds >= 0 && seconds <= duration ? seconds : null
}

function isShortcutTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  return target.isContentEditable || ['INPUT', 'SELECT', 'BUTTON', 'TEXTAREA'].includes(target.tagName)
}

function RepresentativeVideo({ event, registry }: { readonly event: TeslaCamEvent; readonly registry: ObjectUrlRegistry }) {
  const [url, setUrl] = useState<string | null>(null)
  useEffect(() => {
    const fileRef = event.representativeFile
    if (fileRef === undefined) return undefined
    let active = true
    let acquired: File | null = null
    void fileRef.getFile().then(
      (file) => {
        if (!active) return
        acquired = file
        setUrl(registry.acquire(file))
      },
      (error: unknown) => {
        if (error instanceof Error || error instanceof DOMException) return
        throw error
      },
    )
    return () => {
      active = false
      if (acquired !== null) registry.release(acquired)
    }
  }, [event.representativeFile, registry])
  return (
    <section className="representative-video" aria-label="代表映像">
      <div>
        <strong>代表映像（単独再生）</strong>
        <span>6カメラ同期映像ではありません。</span>
      </div>
      {url !== null && <video controls muted playsInline preload="metadata" src={url} />}
    </section>
  )
}

export function Theater({ event, onPrevious, onNext, hasPrevious, hasNext }: TheaterProps) {
  const registry = useMemo(() => new ObjectUrlRegistry(), [])
  const [expandedCamera, setExpandedCamera] = useState<CameraId | null>(null)
  const [statuses, setStatuses] = useState<Readonly<Partial<Record<CameraId, CameraStatus>>>>({})
  const playback = useSynchronizedPlayback(event, expandedCamera)
  const segment = event.segments.at(playback.segmentIndex)
  const updateStatus = useCallback((cameraId: CameraId, status: CameraStatus): void => {
    setStatuses((current) => current[cameraId] === status ? current : { ...current, [cameraId]: status })
  }, [])

  useEffect(() => () => registry.dispose(), [registry])

  useEffect(() => {
    const handleKey = (keyboardEvent: KeyboardEvent): void => {
      if (keyboardEvent.key === 'Escape' && expandedCamera !== null) {
        setExpandedCamera(null)
        keyboardEvent.preventDefault()
        return
      }
      if (isShortcutTarget(keyboardEvent.target)) return
      let handled = true
      switch (keyboardEvent.key) {
        case ' ':
          if (playback.playing) playback.pause()
          else void playback.play()
          break
        case 'ArrowLeft':
          playback.skip(-10)
          break
        case 'ArrowRight':
          playback.skip(10)
          break
        case '1':
        case '2':
        case '3':
        case '4':
        case '5':
        case '6': {
          const cameraId = cameraIds[Number(keyboardEvent.key) - 1]
          if (cameraId !== undefined) setExpandedCamera((current) => current === cameraId ? null : cameraId)
          break
        }
        default:
          handled = false
      }
      if (handled) keyboardEvent.preventDefault()
    }
    window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [expandedCamera, playback])

  const allTerminal = cameraIds.every((cameraId) => terminalStatuses.includes(statuses[cameraId] ?? 'loading'))
  const representativeMode = allTerminal && event.representativeFile !== undefined
  const markerSeconds = representativeMode ? null : detectionSeconds(event, playback.durationSeconds)

  return (
    <section className="theater" aria-label="6カメラシアター">
      <div className="camera-grid" data-expanded={expandedCamera !== null}>
        {cameraIds.map((cameraId) => (
          <CameraTile
            cameraId={cameraId}
            expanded={expandedCamera === cameraId}
            fileRef={segment?.cameras[cameraId]}
            key={cameraId}
            label={cameraLabels[cameraId]}
            onStatus={updateStatus}
            onPlaybackEvent={playback.handleVideoEvent}
            onToggle={() => setExpandedCamera((current) => current === cameraId ? null : cameraId)}
            registerVideo={playback.registerVideo}
            registry={registry}
            unregisterVideo={playback.unregisterVideo}
          />
        ))}
      </div>
      {representativeMode && <RepresentativeVideo event={event} registry={registry} />}
      <PlaybackControls
        detectionSeconds={markerSeconds}
        hasNext={hasNext}
        hasPrevious={hasPrevious}
        onNext={onNext}
        onPrevious={onPrevious}
        playback={playback}
      />
    </section>
  )
}
