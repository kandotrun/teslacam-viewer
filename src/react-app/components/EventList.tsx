import { ImageOff, Video } from 'lucide-react'
import { useEffect, useState } from 'react'

import { cameraIds, type TeslaCamEvent } from '../../teslacam/types'
import {
  categoryLabel,
  eventMatchesFilters,
  eventTime,
  type EventFilters,
} from '../app-state'

type EventListProps = {
  readonly events: readonly TeslaCamEvent[]
  readonly filters: EventFilters
  readonly selectedEventId: string | null
  readonly onSelectEvent: (eventId: string) => void
}

function visibleEvents(
  events: readonly TeslaCamEvent[],
  filters: EventFilters,
): readonly TeslaCamEvent[] {
  const matchingEvents: TeslaCamEvent[] = []
  for (const event of events) {
    if (eventMatchesFilters(event, filters)) matchingEvents.push(event)
  }
  return matchingEvents.sort((left, right) =>
    right.timestamp.localeCompare(left.timestamp),
  )
}

function missingCameraCount(event: TeslaCamEvent): number {
  let maximumMissing = 0
  for (const segment of event.segments) {
    const missing = cameraIds.filter((cameraId) => segment.cameras[cameraId] === undefined)
      .length
    maximumMissing = Math.max(maximumMissing, missing)
  }
  return maximumMissing
}

function EventThumbnail({ event }: { readonly event: TeslaCamEvent }) {
  const [url, setUrl] = useState<string | null>(null)

  useEffect(() => {
    const thumbnailFile = event.thumbnailFile
    if (thumbnailFile === undefined) return undefined

    let mounted = true
    let objectUrl: string | null = null
    void thumbnailFile.getFile().then(
      (file) => {
        if (!mounted) return
        objectUrl = URL.createObjectURL(file)
        setUrl(objectUrl)
      },
      (error: unknown) => {
        if (error instanceof Error || error instanceof DOMException) return
        throw error
      },
    )

    return () => {
      mounted = false
      if (objectUrl !== null) URL.revokeObjectURL(objectUrl)
    }
  }, [event.thumbnailFile])

  if (url === null) {
    return (
      <span className="event-thumbnail event-thumbnail--empty" aria-hidden="true">
        <ImageOff size={20} strokeWidth={1.8} />
      </span>
    )
  }

  return <img alt="" className="event-thumbnail" src={url} />
}

export function EventList({
  events,
  filters,
  selectedEventId,
  onSelectEvent,
}: EventListProps) {
  const matchingEvents = visibleEvents(events, filters)

  return (
    <section className="event-list-panel" aria-labelledby="event-list-title">
      <div className="event-list-panel__header">
        <div>
          <p className="sidebar-label">EVENTS</p>
          <h1 id="event-list-title">イベント</h1>
        </div>
        <span>{matchingEvents.length} 件</span>
      </div>
      {matchingEvents.length === 0 ? (
        <p className="event-list-empty">
          <span>この条件に一致するイベントはありません。</span>
        </p>
      ) : (
        <div className="event-list" aria-label="イベント一覧">
          {matchingEvents.map((event) => {
            const missingCameras = missingCameraCount(event)
            const isSelected = event.id === selectedEventId
            return (
              <button
                aria-pressed={isSelected}
                className="event-card"
                data-selected={isSelected}
                key={event.id}
                type="button"
                onClick={() => onSelectEvent(event.id)}
              >
                <EventThumbnail event={event} />
                <span className="event-card__content">
                  <span className={`event-category event-category--${event.category}`}>
                    <Video aria-hidden="true" size={14} strokeWidth={1.8} />
                    {categoryLabel(event.category)}
                  </span>
                  <strong>{eventTime(event.timestamp)}</strong>
                  <span>{event.segments.length} セグメント</span>
                  <span>{missingCameras}台のカメラがありません</span>
                </span>
              </button>
            )
          })}
        </div>
      )}
    </section>
  )
}
