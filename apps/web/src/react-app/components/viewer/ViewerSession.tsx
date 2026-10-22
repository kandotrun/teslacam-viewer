import { useCallback, useEffect, useMemo, useState } from 'react'

import { cameraIds, type CameraId, type TeslaCamEvent } from '../../../teslacam/types'
import { categoryLabel, eventTime } from '../../event-filters'
import { cameraLabels } from '../../event-summary'
import { ObjectUrlRegistry } from '../../playback/object-url-registry'
import { segmentBuffers } from '../../playback/segment-buffers'
import { useSynchronizedPlayback } from '../../playback/use-synchronized-playback'
import { initialPlaybackSeconds, sentryDetection, startsAtPreRoll } from '../../presentation/event-copy'
import {
  effectiveViewMode,
  revealOrder,
  stageMode,
  viewModesFor,
  windowArea,
  type LayoutBand,
  type ViewMode,
} from '../../presentation/stage-layout'
import { shortcutFocus, theaterShortcut } from '../../theater-shortcuts'
import { ActionsPill } from './ActionsPill'
import { CameraSwitcher } from './CameraSwitcher'
import { terminalStatuses, type CameraStatus } from '../../presentation/camera-status'
import { CameraWindow } from './CameraWindow'
import { EventPill } from './EventPill'
import { RepresentativeWindow } from './RepresentativeWindow'
import { StageFrame } from './StageFrame'
import { Transport } from './Transport'
import { useCompositeExport } from './use-composite-export'

export type ViewerSessionProps = {
  readonly event: TeslaCamEvent
  readonly band: LayoutBand
  readonly viewMode: ViewMode
  readonly onViewMode: (mode: ViewMode) => void
  readonly onOlder: (() => void) | null
  readonly onNewer: (() => void) | null
  readonly onBack?: (() => void) | undefined
  readonly libraryToggle?: { readonly hidden: boolean; readonly onToggle: () => void } | undefined
  /** Fill the windows one by one (the first event after a folder loads). */
  readonly reveal?: boolean
}

/**
 * Everything owned by the selected event: synchronized playback of the six cameras, their windows,
 * the expanded and active camera and the export job (parity spec §0.1 "Theater session").
 */
function Session({ event, band, viewMode, onViewMode, onOlder, onNewer, onBack, libraryToggle, reveal = false }: ViewerSessionProps) {
  const registry = useMemo(() => new ObjectUrlRegistry(), [])
  const [expandedCamera, setExpandedCamera] = useState<CameraId | null>(null)
  const [activeCamera, setActiveCamera] = useState<CameraId>('front')
  const [statuses, setStatuses] = useState<Readonly<Partial<Record<CameraId, CameraStatus>>>>({})
  const preferredCamera = band === 'phone' ? activeCamera : expandedCamera
  const playback = useSynchronizedPlayback(event, preferredCamera, initialPlaybackSeconds(event))
  const exporter = useCompositeExport(event)
  const segment = event.segments.at(playback.segmentIndex)
  const buffers = segmentBuffers(playback.segmentIndex, event.segments.length)
  const spareSegment = buffers.spareSegmentIndex === null ? undefined : event.segments.at(buffers.spareSegmentIndex)
  const everyCameraFailed = cameraIds.every((cameraId) => terminalStatuses.includes(statuses[cameraId] ?? 'loading'))
  const representative = event.representativeFile
  const view = effectiveViewMode(viewMode, representative !== undefined, everyCameraFailed)
  const mode = stageMode(view, expandedCamera, band)
  const focusCamera = mode === 'expanded' ? (expandedCamera ?? activeCamera) : activeCamera
  const detection = sentryDetection(event)
  const exportDialogOpen = exporter.state.status === 'idle' && exporter.state.dialogOpen
  const { pause } = playback

  useEffect(() => () => registry.dispose(), [registry])

  // The representative video has its own controls; the six-camera clock pauses meanwhile.
  useEffect(() => {
    if (view === 'representative') pause()
  }, [pause, view])

  const updateStatus = useCallback((cameraId: CameraId, status: CameraStatus): void => {
    setStatuses((current) => (current[cameraId] === status ? current : { ...current, [cameraId]: status }))
  }, [])

  const toggleCamera = useCallback((cameraId: CameraId): void => {
    setActiveCamera(cameraId)
    if (band === 'phone') return
    setExpandedCamera((current) => (current === cameraId ? null : cameraId))
  }, [band])

  const selectCamera = useCallback((cameraId: CameraId): void => {
    setActiveCamera(cameraId)
    if (band !== 'phone') setExpandedCamera(cameraId)
  }, [band])

  const changeView = useCallback((next: ViewMode): void => {
    setExpandedCamera(null)
    onViewMode(next)
  }, [onViewMode])

  useEffect(() => {
    const handleKey = (keyboard: KeyboardEvent): void => {
      const shortcut = theaterShortcut({
        key: keyboard.key,
        modifiers: { meta: keyboard.metaKey, ctrl: keyboard.ctrlKey, alt: keyboard.altKey },
        focus: shortcutFocus(keyboard.target),
        expandedCamera,
        playing: playback.playing,
        dialogOpen: exportDialogOpen,
        clockSuspended: view === 'representative',
      })
      switch (shortcut.action) {
        case 'none':
          return
        case 'collapse':
          setExpandedCamera(null)
          break
        case 'play':
          void playback.play()
          break
        case 'pause':
          playback.pause()
          break
        case 'skip':
          playback.skip(shortcut.seconds)
          break
        case 'toggleCamera':
          toggleCamera(shortcut.camera)
          break
        case 'olderEvent':
          onOlder?.()
          break
        case 'newerEvent':
          onNewer?.()
          break
      }
      keyboard.preventDefault()
    }
    window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [expandedCamera, exportDialogOpen, onNewer, onOlder, playback, toggleCamera, view])

  return (
    <>
      {/* The screen's one heading names the event on the stage; the pill shows it visually. */}
      <h1 className="visually-hidden">{categoryLabel(event.category)} {eventTime(event.timestamp)}</h1>
      <header className="topbar">
        <EventPill event={event} libraryToggle={libraryToggle} onBack={onBack} />
        <ActionsPill
          event={event}
          exporter={exporter}
          mode={view}
          modes={viewModesFor(representative !== undefined, band)}
          representativeForced={everyCameraFailed && representative !== undefined}
          onMode={changeView}
        />
      </header>
      <StageFrame
        activeCamera={mode === 'spatial' ? activeCamera : null}
        extra={(
          <>
            {mode === 'representative' && representative !== undefined && (
              <RepresentativeWindow fileRef={representative} registry={registry} />
            )}
            {(mode === 'expanded' || mode === 'single') && (
              <CameraSwitcher activeCamera={focusCamera} variant={mode === 'single' ? 'labeled' : 'compact'} onSelect={selectCamera} />
            )}
          </>
        )}
        mode={mode}
        reveal={reveal}
        roomForMessage={playback.message !== null && view !== 'representative'}
      >
        {cameraIds.map((cameraId) => (
          <CameraWindow
            active={cameraId === activeCamera}
            activeSlot={buffers.activeSlot}
            area={windowArea(mode, cameraId, focusCamera)}
            cameraId={cameraId}
            expanded={cameraId === expandedCamera && mode === 'expanded'}
            fileRef={segment?.cameras[cameraId]}
            key={cameraId}
            label={cameraLabels[cameraId]}
            registerVideo={playback.registerVideo}
            registry={registry}
            revealIndex={revealOrder.indexOf(cameraId)}
            spareFileRef={spareSegment?.cameras[cameraId]}
            unregisterVideo={playback.unregisterVideo}
            onPlaybackEvent={playback.handleVideoEvent}
            onStatus={updateStatus}
            onToggle={toggleCamera}
          />
        ))}
      </StageFrame>
      <Transport
        band={band}
        showMessage={view !== 'representative'}
        detectionSeconds={view === 'representative' || detection === null ? null : detection.seconds}
        playback={playback}
        preRollSeconds={startsAtPreRoll(event) ? initialPlaybackSeconds(event) : null}
        onNewer={onNewer}
        onOlder={onOlder}
      />
    </>
  )
}

/** A new session (fresh playback, cameras, export) for every selected event. */
export function ViewerSession(props: ViewerSessionProps) {
  return <Session key={props.event.id} {...props} />
}
