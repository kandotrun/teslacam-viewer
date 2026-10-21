import { ChevronLeft, ChevronRight, History } from 'lucide-react'
import { useState } from 'react'

import type { TeslaCamEvent } from '../../teslacam/types'
import { categoryLabel } from '../event-filters'
import { useTimelinePresentation } from '../use-timeline-presentation'
import {
  adjacentTimelineDays,
  buildDayTimelineClusters,
  buildDayTimelineMarkers,
  buildTimelineDays,
  selectTimelineDate,
  type DayTimelineMarker,
  type TimelineDay,
} from '../day-timeline'
import { DayTimelineLedger } from './DayTimelineLedger'
import { DayTimelineRail } from './DayTimelineRail'

export type TimelineDateSelection = {
  readonly dateKey: string
  readonly eventId: string
}

type DayTimelineProps = {
  readonly events: readonly TeslaCamEvent[]
  readonly filterDateKey: string | null
  readonly selectedEvent: TeslaCamEvent | undefined
  readonly selectedEventId: string | null
  readonly onSelectDate: (selection: TimelineDateSelection) => void
  readonly onSelectEvent: (eventId: string) => void
}

const minimumDesktopTrackWidth = 640

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

function restoreSelectedMarkerFocus(eventId: string): void {
  requestAnimationFrame(() => {
    const selectedButton = Array.from(
      document.querySelectorAll<HTMLButtonElement>('[data-day-event-id]'),
    ).find((button) => button.dataset['dayEventId'] === eventId)
    if (selectedButton !== undefined) {
      selectedButton.focus()
      return
    }
    const clusterButton = Array.from(
      document.querySelectorAll<HTMLButtonElement>('[data-day-cluster-event-ids]'),
    ).find((button) => button.dataset['dayClusterEventIds']?.split(' ').includes(eventId))
    clusterButton?.focus()
  })
}

export function DayTimeline({
  events,
  filterDateKey,
  selectedEvent,
  selectedEventId,
  onSelectDate,
  onSelectEvent,
}: DayTimelineProps) {
  const presentation = useTimelinePresentation()
  const [mobileHistoryOpen, setMobileHistoryOpen] = useState(false)
  const [expandedClusterId, setExpandedClusterId] = useState<string | null>(null)
  const dateKey = selectTimelineDate(events, selectedEvent, filterDateKey)
  const markers = buildDayTimelineMarkers(events, dateKey)
  const clusters = buildDayTimelineClusters(events, dateKey, minimumDesktopTrackWidth)
  const days = buildTimelineDays(events)
  const adjacentDays = adjacentTimelineDays(days, dateKey)
  const selectedIndex = markerIndex(markers, selectedEventId)
  const selectedMarker = selectedIndex < 0 ? undefined : markers[selectedIndex]
  const previousMarker = selectedIndex <= 0 ? undefined : markers[selectedIndex - 1]
  const nextMarker = selectedIndex < 0 ? undefined : markers[selectedIndex + 1]
  const clusterExpanded = presentation === 'rail'
    && clusters.some((cluster) => cluster.id === expandedClusterId)

  function selectMarker(eventId: string): void {
    onSelectEvent(eventId)
    restoreSelectedMarkerFocus(eventId)
  }

  function selectDay(day: TimelineDay | undefined): void {
    const eventId = day?.eventIds.at(-1)
    if (day === undefined || eventId === undefined) return
    onSelectDate({ dateKey: day.dateKey, eventId })
  }

  return (
    <section
      className="day-timeline"
      aria-label="日内イベントタイムライン"
      data-cluster-expanded={clusterExpanded}
      data-presentation={presentation}
    >
      <header className="day-timeline__header">
        <div className="day-timeline__heading">
          <h2>日内イベント</h2>
          <p>{dateKey ?? '日付を選択してください'}</p>
        </div>
        <nav className="day-timeline__date-nav" aria-label="記録日を移動">
          <button
            aria-label={adjacentDays.previous === undefined
              ? '前の記録日'
              : `前の記録日 ${adjacentDays.previous.dateKey}`}
            disabled={adjacentDays.previous === undefined}
            type="button"
            onClick={() => selectDay(adjacentDays.previous)}
          >
            <ChevronLeft aria-hidden="true" size={16} strokeWidth={1.75} />
            <span>前の記録日</span>
          </button>
          <button
            aria-label={adjacentDays.next === undefined
              ? '次の記録日'
              : `次の記録日 ${adjacentDays.next.dateKey}`}
            disabled={adjacentDays.next === undefined}
            type="button"
            onClick={() => selectDay(adjacentDays.next)}
          >
            <span>次の記録日</span>
            <ChevronRight aria-hidden="true" size={16} strokeWidth={1.75} />
          </button>
        </nav>
      </header>
      {markers.length === 0 ? (
        <p className="day-timeline__empty">この日のイベントはありません。</p>
      ) : (
        <>
          {presentation === 'rail' && (
            <DayTimelineRail
              clusters={clusters}
              expandedClusterId={expandedClusterId}
              markers={markers}
              selectedEventId={selectedEventId}
              onExpandedCluster={setExpandedClusterId}
              onSelectEvent={selectMarker}
            />
          )}
          {presentation === 'ledger' && (
            <DayTimelineLedger
              markers={markers}
              selectedEventId={selectedEventId}
              onSelectEvent={selectMarker}
            />
          )}
          {presentation === 'mobile' && (
            <div className="day-timeline__mobile-history">
              <button
                aria-expanded={mobileHistoryOpen}
                className="day-timeline__history-trigger"
                type="button"
                onClick={() => setMobileHistoryOpen((open) => !open)}
              >
                <History aria-hidden="true" size={16} strokeWidth={1.75} />
                {`${mobileHistoryOpen ? '履歴を閉じる' : '履歴を開く'}（${markers.length}件）`}
              </button>
              {mobileHistoryOpen && (
                <DayTimelineLedger
                  markers={markers}
                  selectedEventId={selectedEventId}
                  onSelectEvent={selectMarker}
                />
              )}
            </div>
          )}
          <div className="day-timeline__summary" aria-label="イベントの前後関係">
            <p aria-label="過去の記録">過去: {markerSummary(previousMarker)}</p>
            <p aria-label="選択中の記録">選択: {markerSummary(selectedMarker)}</p>
            <p aria-label="新しい記録">新しい: {markerSummary(nextMarker)}</p>
            <p aria-label="この日の記録数">同日: {markers.length}件</p>
          </div>
        </>
      )}
    </section>
  )
}
