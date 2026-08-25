import { useRef, type KeyboardEvent } from 'react'

import type { DayTimelineMarker } from '../day-timeline'
import { TimelineEventButton } from './TimelineEventButton'

type DayTimelineLedgerProps = {
  readonly markers: readonly DayTimelineMarker[]
  readonly selectedEventId: string | null
  readonly onSelectEvent: (eventId: string) => void
}

export function DayTimelineLedger({
  markers,
  selectedEventId,
  onSelectEvent,
}: DayTimelineLedgerProps) {
  const eventButtons = useRef<Array<HTMLButtonElement | null>>([])

  function handleKeyDown(event: KeyboardEvent<HTMLButtonElement>, eventId: string): void {
    const index = markers.findIndex((marker) => marker.eventId === eventId)
    if (index < 0) return
    const targetIndex = event.key === 'Home'
      ? 0
      : event.key === 'End'
        ? markers.length - 1
        : event.key === 'ArrowLeft' || event.key === 'ArrowUp'
          ? index - 1
          : event.key === 'ArrowRight' || event.key === 'ArrowDown'
            ? index + 1
            : -1
    const target = eventButtons.current.at(targetIndex)
    if (target === undefined || target === null) return
    event.preventDefault()
    target.focus()
  }

  return (
    <div className="day-timeline__ledger" role="group" aria-label="この日の記録">
      {markers.map((marker, index) => (
        <TimelineEventButton
          buttonRef={(button) => {
            eventButtons.current[index] = button
          }}
          className="day-timeline__ledger-event"
          key={marker.eventId}
          marker={marker}
          selected={marker.eventId === selectedEventId}
          onKeyDown={handleKeyDown}
          onSelect={onSelectEvent}
        />
      ))}
    </div>
  )
}
