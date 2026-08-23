import { useCallback, useEffect, useMemo, useState, type RefObject } from 'react'

import { cameraIds, type CameraId, type TeslaCamEvent } from '../../teslacam/types'
import { detectionSeconds } from '../event-detection'
import { cameraLabels } from '../event-summary'
import { ObjectUrlRegistry } from '../playback/object-url-registry'
import { useSynchronizedPlayback } from '../playback/use-synchronized-playback'
import { useTheaterShortcuts } from '../use-theater-shortcuts'
import { CameraTile, type CameraStatus } from './CameraTile'
import { PlaybackControls } from './PlaybackControls'
import { RepresentativeVideo } from './RepresentativeVideo'
import { TheaterContext } from './TheaterContext'

export type TheaterProps = {
  readonly event: TeslaCamEvent
  readonly onPrevious: () => void
  readonly onNext: () => void
  readonly hasPrevious: boolean
  readonly hasNext: boolean
  readonly sourceName?: string
  readonly theaterRef?: RefObject<HTMLElement | null>
}

const terminalStatuses: readonly CameraStatus[] = ['missing', 'unavailable', 'corrupt', 'unsupported']

function TheaterSession({ event, onPrevious, onNext, hasPrevious, hasNext, sourceName = 'TeslaCam', theaterRef }: TheaterProps) {
  const registry = useMemo(() => new ObjectUrlRegistry(), [])
  const [expandedCamera, setExpandedCamera] = useState<CameraId | null>(null)
  const [activeCamera, setActiveCamera] = useState<CameraId>('front')
  const [statuses, setStatuses] = useState<Readonly<Partial<Record<CameraId, CameraStatus>>>>({})
  const playback = useSynchronizedPlayback(event, expandedCamera)
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
  const markerSeconds = representativeMode ? null : detectionSeconds(event, playback.durationSeconds)

  return (
    <section className="theater" aria-label="6カメラシアター" data-representative={representativeMode} ref={theaterRef}>
      <TheaterContext event={event} sourceName={sourceName} />
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
      <div className="camera-grid" data-expanded={expandedCamera !== null}>
        {cameraIds.map((cameraId) => (
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

export function Theater(props: TheaterProps) {
  return <TheaterSession key={props.event.id} {...props} />
}
