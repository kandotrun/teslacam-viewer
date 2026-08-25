import { useCallback, useEffect, useMemo, useState, type RefObject } from 'react'

import { cameraIds, type CameraId, type TeslaCamEvent } from '../../teslacam/types'
import { detectionSeconds, sentryInitialElapsedSeconds } from '../event-detection'
import { cameraLabels } from '../event-summary'
import { ObjectUrlRegistry } from '../playback/object-url-registry'
import { useSynchronizedPlayback } from '../playback/use-synchronized-playback'
import { useTheaterShortcuts } from '../use-theater-shortcuts'
import { CameraTile, type CameraStatus } from './CameraTile'
import { CompositeVideoExport } from './CompositeVideoExport'
import { DayTimeline, type TimelineDateSelection } from './DayTimeline'
import { EventInspector } from './EventInspector'
import { PlaybackControls } from './PlaybackControls'
import { RepresentativeVideo } from './RepresentativeVideo'
import { TheaterContext } from './TheaterContext'
import { TheaterViewSwitch, type TheaterViewMode } from './TheaterViewSwitch'

export type TheaterProps = {
  readonly event: TeslaCamEvent
  readonly onPrevious: () => void
  readonly onNext: () => void
  readonly hasPrevious: boolean
  readonly hasNext: boolean
  readonly sourceName?: string
  readonly theaterRef?: RefObject<HTMLElement | null>
  readonly timelineEvents?: readonly TeslaCamEvent[]
  readonly timelineDateKey?: string | null
  readonly selectedEventId?: string | null
  readonly onSelectTimelineDate?: (selection: TimelineDateSelection) => void
  readonly onSelectEvent?: (eventId: string) => void
  readonly viewMode: TheaterViewMode
  readonly onViewMode: (mode: TheaterViewMode) => void
}

const terminalStatuses: readonly CameraStatus[] = ['missing', 'unavailable', 'corrupt', 'unsupported']

function ignoreEventSelection(_eventId: string): void {
  return undefined
}

function ignoreTimelineDateSelection(_selection: TimelineDateSelection): void {
  return undefined
}

function TheaterSession({
  event,
  onPrevious,
  onNext,
  hasPrevious,
  hasNext,
  sourceName = 'TeslaCam',
  theaterRef,
  timelineEvents = [event],
  timelineDateKey = null,
  selectedEventId = event.id,
  onSelectTimelineDate = ignoreTimelineDateSelection,
  onSelectEvent = ignoreEventSelection,
  viewMode,
  onViewMode,
}: TheaterProps) {
  const registry = useMemo(() => new ObjectUrlRegistry(), [])
  const [expandedCamera, setExpandedCamera] = useState<CameraId | null>(null)
  const [activeCamera, setActiveCamera] = useState<CameraId>('front')
  const [statuses, setStatuses] = useState<Readonly<Partial<Record<CameraId, CameraStatus>>>>({})
  const detectedSeconds = detectionSeconds(event)
  const initialElapsedSeconds = sentryInitialElapsedSeconds(event)
  const playback = useSynchronizedPlayback(event, expandedCamera, initialElapsedSeconds)
  const segment = event.segments.at(playback.segmentIndex)
  const updateStatus = useCallback((cameraId: CameraId, status: CameraStatus): void => {
    setStatuses((current) => current[cameraId] === status ? current : { ...current, [cameraId]: status })
  }, [])

  useEffect(() => () => registry.dispose(), [registry])

  useTheaterShortcuts({
    expandedCamera,
    setExpandedCamera,
    setActiveCamera,
    playing: playback.playing,
    play: playback.play,
    pause: playback.pause,
    skip: playback.skip,
  })

  const allTerminal = cameraIds.every((cameraId) => terminalStatuses.includes(statuses[cameraId] ?? 'loading'))
  const representativeMode = allTerminal && event.representativeFile !== undefined
  const markerSeconds = representativeMode ? null : detectedSeconds
  const cameraTiles = cameraIds.map((cameraId) => (
    <CameraTile
      cameraId={cameraId}
      active={activeCamera === cameraId}
      expanded={expandedCamera === cameraId}
      fileRef={segment?.cameras[cameraId]}
      key={cameraId}
      label={cameraLabels[cameraId]}
      onStatus={updateStatus}
      onPlaybackEvent={playback.handleVideoEvent}
      onToggle={() => {
        setActiveCamera(cameraId)
        setExpandedCamera((current) => current === cameraId ? null : cameraId)
      }}
      registerVideo={playback.registerVideo}
      registry={registry}
      unregisterVideo={playback.unregisterVideo}
    />
  ))

  return (
    <section className="theater" aria-label="6カメラシアター" data-representative={representativeMode} ref={theaterRef}>
      <TheaterContext event={event} sourceName={sourceName} />
      <div className="theater-toolbar">
        <CompositeVideoExport event={event} />
        {!representativeMode && (
          <TheaterViewSwitch
            disabled={expandedCamera !== null}
            mode={viewMode}
            onChange={onViewMode}
          />
        )}
      </div>
      <div className="camera-switcher" role="group" aria-label="表示するカメラ">
        {cameraIds.map((cameraId) => (
          <button
            aria-label={`${cameraLabels[cameraId]}を表示`}
            aria-pressed={activeCamera === cameraId}
            key={cameraId}
            type="button"
            onClick={() => {
              setActiveCamera(cameraId)
              setExpandedCamera(null)
            }}
          >
            {cameraLabels[cameraId]}
          </button>
        ))}
      </div>
      <div className="theater-stage-row">
        <div className="camera-grid camera-stage" data-expanded={expandedCamera !== null} data-view-mode={viewMode}>
          {viewMode === 'spatial' && <img alt="" height="240" src="/vehicle-top.png" width="120" />}
          {cameraTiles}
        </div>
        <EventInspector event={event} />
      </div>
      {representativeMode && <RepresentativeVideo event={event} registry={registry} />}
      <div className="playback-deck">
        <PlaybackControls
          detectionSeconds={markerSeconds}
          hasNext={hasNext}
          hasPrevious={hasPrevious}
          onNext={onNext}
          onPrevious={onPrevious}
          playback={playback}
        />
        <DayTimeline
          events={timelineEvents}
          filterDateKey={timelineDateKey}
          selectedEvent={event}
          selectedEventId={selectedEventId}
          onSelectDate={onSelectTimelineDate}
          onSelectEvent={onSelectEvent}
        />
      </div>
    </section>
  )
}

export function Theater(props: TheaterProps) {
  return <TheaterSession key={props.event.id} {...props} />
}
