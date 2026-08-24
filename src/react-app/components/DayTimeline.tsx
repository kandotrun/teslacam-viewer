import { useRef } from 'react'

import type { TeslaCamEvent } from '../../teslacam/types'
import { categoryLabel } from '../event-filters'
import {
  buildDayTimelineMarkers,
  selectTimelineDate,
  type DayTimelineMarker,
} from '../day-timeline'

type DayTimelineProps = {
  readonly events: readonly TeslaCamEvent[]
  readonly filterDateKey: string | null
  readonly selectedEvent: TeslaCamEvent | undefined
  readonly selectedEventId: string | null
  readonly onSelectEvent: (eventId: string) => void
}

function markerSummary(marker: DayTimelineMarker | undefined): string {
  if (marker === undefined) return 'なし'
  return `${marker.timeLabel} ${categoryLabel(marker.category)}`
}

function markerIndex(
  markers: readonly DayTimelineMarker[],
  selectedEventId: string | null,
): number {
  if (selectedEventId === null) return -1
  return markers.findIndex((marker) => marker.eventId === selectedEventId)
}

export function DayTimeline({
  events,
  filterDateKey,
  selectedEvent,
  selectedEventId,
  onSelectEvent,
}: DayTimelineProps) {
  const markerButtons = useRef<Array<HTMLButtonElement | null>>([])
  const dateKey = selectTimelineDate(events, selectedEvent, filterDateKey)
  const markers = buildDayTimelineMarkers(events, dateKey)
  const selectedIndex = markerIndex(markers, selectedEventId)
  const selectedMarker = selectedIndex < 0 ? undefined : markers[selectedIndex]
  const previousMarker = selectedIndex <= 0 ? undefined : markers[selectedIndex - 1]
  const nextMarker = selectedIndex < 0 ? undefined : markers[selectedIndex + 1]

  function restoreSelectedMarkerFocus(eventId: string): void {
    requestAnimationFrame(() => {
      const selectedButton = Array.from(
        document.querySelectorAll<HTMLButtonElement>('[data-day-event-id]'),
      ).find((button) => button.dataset['dayEventId'] === eventId)
      selectedButton?.focus()
    })
  }

  function selectMarker(eventId: string): void {
    onSelectEvent(eventId)
    restoreSelectedMarkerFocus(eventId)
  }

  function moveFocus(index: number, direction: -1 | 1): void {
    const nextIndex = index + direction
    markerButtons.current[nextIndex]?.focus()
  }

  return (
    <section className="day-timeline" aria-label="日内イベントタイムライン">
      <header className="day-timeline__header">
        <h2>日内イベント</h2>
        <p>{dateKey ?? '日付を選択してください'}</p>
      </header>
      {markers.length === 0 ? (
        <p className="day-timeline__empty">この日のイベントはありません。</p>
      ) : (
        <>
          <div className="day-timeline__ruler" aria-hidden="true">
            <span>00:00</span>
            <span>12:00</span>
            <span>24:00</span>
          </div>
          <div className="day-timeline__markers" aria-label={`${dateKey} のイベント`}>
            {markers.map((marker, index) => {
              const isSelected = marker.eventId === selectedEventId
              return (
                <button
                  aria-label={`${marker.dateKey} ${marker.timeLabel} ${categoryLabel(marker.category)}`}
                  aria-pressed={isSelected}
                  className="day-timeline__marker"
                  data-category={marker.category}
                  data-day-event-id={marker.eventId}
                  data-selected={isSelected}
                  key={marker.eventId}
                  ref={(button) => {
                    markerButtons.current[index] = button
                  }}
                  style={{ insetInlineStart: `${marker.positionPercent}%` }}
                  type="button"
                  onClick={() => selectMarker(marker.eventId)}
                  onKeyDown={(keyboardEvent) => {
                    if (keyboardEvent.key === 'ArrowLeft' && index > 0) {
                      keyboardEvent.preventDefault()
                      moveFocus(index, -1)
                    }
                    if (keyboardEvent.key === 'ArrowRight' && index < markers.length - 1) {
                      keyboardEvent.preventDefault()
                      moveFocus(index, 1)
                    }
                  }}
                >
                  <span className="day-timeline__marker-time">{marker.timeLabel}</span>
                  <span className="day-timeline__marker-category">{categoryLabel(marker.category)}</span>
                </button>
              )
            })}
          </div>
          <div className="day-timeline__summary" aria-label="イベントの前後関係">
            <p aria-label="前のイベント">前: {markerSummary(previousMarker)}</p>
            <p aria-label="選択中のイベント">選択: {markerSummary(selectedMarker)}</p>
            <p aria-label="次のイベント">次: {markerSummary(nextMarker)}</p>
            <p aria-label="この日のイベント数">同日: {markers.length}件</p>
          </div>
        </>
      )}
    </section>
  )
}
