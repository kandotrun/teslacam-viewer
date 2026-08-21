import { ImageOff, Video } from 'lucide-react'
import { useEffect, useState } from 'react'

import type { TeslaCamEvent } from '../../teslacam/types'
import type { EventFilters } from '../app-state'

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
  const query = filters.query.trim().toLowerCase()
  const matchingEvents: TeslaCamEvent[] = []
  for (const event of events) {
    if (filters.category !== 'all' && event.category !== filters.category) continue
    if (filters.dateKey !== null && !event.timestamp.startsWith(filters.dateKey)) continue
    if (
      query !== '' &&
      !`${eventTime(event.timestamp)} ${event.category}`.toLowerCase().includes(query)
    ) {
      continue
    }
    matchingEvents.push(event)
  }
  return matchingEvents.sort((left, right) =>
    right.timestamp.localeCompare(left.timestamp),
  )
}

function categoryLabel(category: TeslaCamEvent['category']): string {
  switch (category) {
    case 'recent':
      return '最近の走行'
    case 'saved':
      return '保存済み'
    case 'sentry':
      return 'セントリー'
  }
}

function cameraCount(event: TeslaCamEvent): number {
  const cameras = new Set<string>()
  for (const segment of event.segments) {
    for (const camera of Object.keys(segment.cameras)) cameras.add(camera)
  }
  return cameras.size
}

function eventTime(timestamp: string): string {
  return timestamp.replace('_', ' ').replaceAll('-', (match, offset) =>
    offset > 10 ? ':' : match,
  )
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
        <p className="event-list-empty">この条件に一致するイベントはありません。</p>
      ) : (
        <div className="event-list" aria-label="イベント一覧">
          {matchingEvents.map((event) => {
            const availableCameras = cameraCount(event)
            const missingCameras = Math.max(0, 6 - availableCameras)
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
