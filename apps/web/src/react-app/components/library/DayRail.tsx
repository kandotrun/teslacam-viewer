import { useRef, type KeyboardEvent } from 'react'

import type { TeslaCamEvent } from '../../../teslacam/types'
import { buildDayTimelineClusters, type DayTimelineCluster, type DayTimelineMarker } from '../../day-timeline'
import { categoryLabel } from '../../event-filters'
import { useElementSize } from '../../hooks/use-element-size'
import { clusterRange, layoutRailItems, railPillsFit } from '../../presentation/rail-layout'

type DayRailProps = {
  readonly events: readonly TeslaCamEvent[]
  readonly dayKey: string
  readonly markers: readonly DayTimelineMarker[]
  readonly selectedEventId: string | null
  readonly expandedClusterId: string | null
  readonly onSelectEvent: (eventId: string) => void
  readonly onToggleCluster: (cluster: DayTimelineCluster) => void
}

const markerHitWidth = 44
const fullPillWidth = 104
const compactPillWidth = 44

function centerPercent(cluster: DayTimelineCluster): number {
  return (cluster.startPositionPercent + cluster.endPositionPercent) / 2
}

/**
 * The 0–24 h rail of the day on screen. Markers whose 44 px hit areas overlap on the measured track
 * become one cluster pill (DESIGN.md §6 Day rail).
 */
export function DayRail({ events, dayKey, markers, selectedEventId, expandedClusterId, onSelectEvent, onToggleCluster }: DayRailProps) {
  const trackRef = useRef<HTMLDivElement>(null)
  const items = useRef<(HTMLButtonElement | null)[]>([])
  const { width } = useElementSize(trackRef)
  const clusters = buildDayTimelineClusters(events, dayKey, width)
  const fullPills = railPillsFit(clusters.map((cluster) => (cluster.count === 1 ? markerHitWidth : fullPillWidth)), width)
  const itemWidths = clusters.map((cluster) => {
    if (cluster.count === 1) return markerHitWidth
    return fullPills ? fullPillWidth : compactPillWidth
  })
  const lefts = width > 0
    ? layoutRailItems(clusters.map((cluster, index) => ({ center: centerPercent(cluster) / 100 * width, width: itemWidths[index] ?? markerHitWidth })), width)
    : null
  const clustered = new Set(clusters.filter((cluster) => cluster.count > 1).flatMap((cluster) => cluster.markers.map((marker) => marker.eventId)))

  function moveFocus(event: KeyboardEvent<HTMLButtonElement>, index: number): void {
    const target = event.key === 'ArrowLeft'
      ? index - 1
      : event.key === 'ArrowRight'
        ? index + 1
        : event.key === 'Home'
          ? 0
          : event.key === 'End'
            ? clusters.length - 1
            : null
    if (target === null || target < 0 || target >= clusters.length) return
    event.preventDefault()
    items.current[target]?.focus()
  }

  return (
    <div aria-label="この日の記録" className="rail" role="group">
      <div className="rail__track" ref={trackRef}>
        <span aria-hidden="true" className="rail__line" />
        {markers.filter((marker) => !clustered.has(marker.eventId)).map((marker) => (
          <span
            aria-hidden="true"
            className="rail__mark"
            data-category={marker.category}
            data-current={marker.eventId === selectedEventId}
            key={marker.eventId}
            style={{ left: `${marker.positionPercent}%` }}
          />
        ))}
        {clusters.map((cluster, index) => {
          const itemWidth = itemWidths[index] ?? markerHitWidth
          const left = lefts?.[index]
          const position = left === undefined
            ? { left: `calc(${centerPercent(cluster)}% - ${itemWidth / 2}px)`, width: `${itemWidth}px` }
            : { left: `${left}px`, width: `${itemWidth}px` }
          const setItem = (button: HTMLButtonElement | null): void => {
            items.current[index] = button
          }
          const marker = cluster.markers[0]
          if (cluster.count === 1 && marker !== undefined) {
            return (
              <button
                aria-label={`${marker.dateKey} ${marker.timeLabel} ${categoryLabel(marker.category)}`}
                aria-pressed={marker.eventId === selectedEventId}
                className="rail__hit"
                data-day-event-id={marker.eventId}
                key={cluster.id}
                ref={setItem}
                style={position}
                type="button"
                onClick={() => onSelectEvent(marker.eventId)}
                onKeyDown={(event) => moveFocus(event, index)}
              />
            )
          }
          const expanded = cluster.id === expandedClusterId
          const range = clusterRange(cluster)
          return (
            <button
              aria-expanded={expanded}
              aria-label={`${range}・${cluster.count}件を${expanded ? '閉じる' : '開く'}`}
              className="rail__cluster"
              data-compact={!fullPills}
              data-current={cluster.markers.some((candidate) => candidate.eventId === selectedEventId)}
              data-day-cluster-event-ids={cluster.markers.map((candidate) => candidate.eventId).join(' ')}
              key={cluster.id}
              ref={setItem}
              style={position}
              type="button"
              onClick={() => onToggleCluster(cluster)}
              onKeyDown={(event) => moveFocus(event, index)}
            >
              <span aria-hidden="true" className="rail__cluster-dots">
                {cluster.markers.slice(0, 3).map((candidate) => (
                  <span data-category={candidate.category} key={candidate.eventId} />
                ))}
              </span>
              {fullPills && <span className="num rail__cluster-range">{range}</span>}
              <span className="num rail__cluster-count">{cluster.count}</span>
            </button>
          )
        })}
      </div>
      <div aria-hidden="true" className="rail__hours">
        {['0', '6', '12', '18', '24'].map((hour) => <span className="num" key={hour}>{hour}</span>)}
      </div>
    </div>
  )
}
