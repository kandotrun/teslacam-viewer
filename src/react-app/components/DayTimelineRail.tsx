import { useRef, type KeyboardEvent } from 'react'

import type { DayTimelineCluster, DayTimelineMarker } from '../day-timeline'
import { TimelineEventButton } from './TimelineEventButton'

type DayTimelineRailProps = {
  readonly clusters: readonly DayTimelineCluster[]
  readonly expandedClusterId: string | null
  readonly markers: readonly DayTimelineMarker[]
  readonly selectedEventId: string | null
  readonly onExpandedCluster: (clusterId: string | null) => void
  readonly onSelectEvent: (eventId: string) => void
}

function shortTime(timeLabel: string): string {
  return timeLabel.slice(0, 5)
}

function clusterPosition(cluster: DayTimelineCluster): number {
  return (cluster.startPositionPercent + cluster.endPositionPercent) / 2
}

export function DayTimelineRail({
  clusters,
  expandedClusterId,
  markers,
  selectedEventId,
  onExpandedCluster,
  onSelectEvent,
}: DayTimelineRailProps) {
  const eventButtons = useRef(new Map<string, HTMLButtonElement>())
  const clusterButtons = useRef(new Map<string, HTMLButtonElement>())
  const expandedCluster = clusters.find((cluster) => cluster.id === expandedClusterId)

  function focusMarker(eventId: string): void {
    const eventButton = eventButtons.current.get(eventId)
    if (eventButton !== undefined) {
      eventButton.focus()
      return
    }
    const cluster = clusters.find((candidate) => (
      candidate.markers.some((marker) => marker.eventId === eventId)
    ))
    if (cluster !== undefined) clusterButtons.current.get(cluster.id)?.focus()
  }

  function handleMarkerKeyDown(
    event: KeyboardEvent<HTMLButtonElement>,
    eventId: string,
  ): void {
    const index = markers.findIndex((marker) => marker.eventId === eventId)
    if (index < 0) return
    const targetIndex = event.key === 'Home'
      ? 0
      : event.key === 'End'
        ? markers.length - 1
        : event.key === 'ArrowLeft'
          ? index - 1
          : event.key === 'ArrowRight'
            ? index + 1
            : -1
    const target = markers.at(targetIndex)
    if (target === undefined) return
    event.preventDefault()
    focusMarker(target.eventId)
  }

  function setEventButton(eventId: string, button: HTMLButtonElement | null): void {
    if (button === null) {
      eventButtons.current.delete(eventId)
      return
    }
    eventButtons.current.set(eventId, button)
  }

  return (
    <>
      {expandedCluster !== undefined && (
        <div
          className="day-timeline__cluster-list"
          role="group"
          aria-label={`${shortTime(expandedCluster.startTimeLabel)}–${shortTime(expandedCluster.endTimeLabel)}・${expandedCluster.count}件`}
        >
          {expandedCluster.markers.map((marker) => (
            <TimelineEventButton
              buttonRef={(button) => setEventButton(marker.eventId, button)}
              className="day-timeline__cluster-event"
              key={marker.eventId}
              marker={marker}
              selected={marker.eventId === selectedEventId}
              onKeyDown={handleMarkerKeyDown}
              onSelect={onSelectEvent}
            />
          ))}
        </div>
      )}
      <div className="day-timeline__ruler" aria-hidden="true">
        <span>00:00</span>
        <span>12:00</span>
        <span>24:00</span>
      </div>
      <div className="day-timeline__markers" aria-label="この日の記録">
        {clusters.map((cluster) => {
          const position = clusterPosition(cluster)
          if (cluster.count === 1) {
            const marker = cluster.markers[0]
            if (marker === undefined) return null
            return (
              <div
                className="day-timeline__point"
                key={cluster.id}
                style={{ insetInlineStart: `${position}%` }}
              >
                <TimelineEventButton
                  buttonRef={(button) => setEventButton(marker.eventId, button)}
                  className="day-timeline__marker"
                  marker={marker}
                  selected={marker.eventId === selectedEventId}
                  onKeyDown={handleMarkerKeyDown}
                  onSelect={onSelectEvent}
                />
              </div>
            )
          }

          const expanded = expandedClusterId === cluster.id
          const selected = cluster.markers.some((marker) => marker.eventId === selectedEventId)
          const rangeLabel = `${shortTime(cluster.startTimeLabel)}–${shortTime(cluster.endTimeLabel)}・${cluster.count}件`
          return (
            <div
              className="day-timeline__cluster"
              key={cluster.id}
              style={{ insetInlineStart: `${position}%` }}
            >
              <button
                aria-expanded={expanded}
                aria-label={`${rangeLabel}を${expanded ? '閉じる' : '開く'}`}
                className="day-timeline__cluster-trigger"
                data-day-cluster-event-ids={cluster.markers.map((marker) => marker.eventId).join(' ')}
                data-selected={selected}
                ref={(button) => {
                  if (button === null) clusterButtons.current.delete(cluster.id)
                  else clusterButtons.current.set(cluster.id, button)
                }}
                type="button"
                onClick={() => onExpandedCluster(expanded ? null : cluster.id)}
              >
                <span>{`${shortTime(cluster.startTimeLabel)}–${shortTime(cluster.endTimeLabel)}`}</span>
                <span>{cluster.count}件</span>
              </button>
            </div>
          )
        })}
      </div>
    </>
  )
}
