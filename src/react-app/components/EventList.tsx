import { ChevronDown, Video } from 'lucide-react'
import { useEffect, useState } from 'react'

import type { TeslaCamEvent } from '../../teslacam/types'
import { categoryLabel, eventTime } from '../event-filters'
import {
  cameraAvailability,
  cameraAvailabilityLabel,
  eventDurationLabel,
  missingCameraLabel,
  segmentCountLabel,
} from '../event-summary'

type EventListProps = {
  readonly events: readonly TeslaCamEvent[]
  readonly selectedEventId: string | null
  readonly onSelectEvent: (eventId: string) => void
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
      () => {
        if (!mounted) return
        setUrl(null)
      },
    )

    return () => {
      mounted = false
      if (objectUrl !== null) URL.revokeObjectURL(objectUrl)
    }
  }, [event.thumbnailFile])

  if (url === null) return null

  return <img alt="" className="event-thumbnail" src={url} />
}

export function EventList({
  events,
  selectedEventId,
  onSelectEvent,
}: EventListProps) {
  const [collapsed, setCollapsed] = useState(false)

  return (
    <section className="event-list-panel" aria-labelledby="event-list-title" data-collapsed={collapsed}>
      <div className="event-list-panel__header">
        <div>
          <h1 id="event-list-title">イベント</h1>
        </div>
        <div className="event-list-panel__summary">
          <span>{events.length}件</span>
          <button
            aria-controls="event-list-content"
            aria-expanded={!collapsed}
            aria-label={`イベント一覧を${collapsed ? '開く' : '閉じる'}`}
            className="event-list-disclosure"
            type="button"
            onClick={() => setCollapsed((current) => !current)}
          >
            <ChevronDown aria-hidden="true" size={18} />
          </button>
        </div>
      </div>
      {events.length === 0 ? (
        <p className="event-list-empty">
          <span>この条件に一致するイベントはありません。</span>
        </p>
      ) : (
        <div className="event-list" id="event-list-content" aria-label="イベント一覧">
          {events.map((event) => {
            const availability = cameraAvailability(event)
            const missing = missingCameraLabel(availability)
            const isSelected = event.id === selectedEventId
            return (
              <button
                aria-pressed={isSelected}
                className="event-card"
                data-selected={isSelected}
                key={event.id}
                type="button"
                onClick={() => {
                  setCollapsed(true)
                  onSelectEvent(event.id)
                }}
              >
                <EventThumbnail event={event} />
                <span className="event-card__content">
                  <span className="event-card__heading">
                    <strong>{eventTime(event.timestamp)}</strong>
                    <span className={`event-category event-category--${event.category}`}>
                      <Video aria-hidden="true" size={14} strokeWidth={1.8} />
                      {categoryLabel(event.category)}
                    </span>
                  </span>
                  <span className="event-card__facts">
                    <span>{eventDurationLabel(event)}</span>
                    <span>{segmentCountLabel(event)}</span>
                    <span>{cameraAvailabilityLabel(availability)}</span>
                  </span>
                  {missing !== null && <span className="event-card__warning">{missing}</span>}
                </span>
              </button>
            )
          })}
        </div>
      )}
    </section>
  )
}
