import type { KeyboardEvent } from 'react'

import { categoryLabel } from '../event-filters'
import type { DayTimelineMarker } from '../day-timeline'

type TimelineEventButtonProps = {
  readonly marker: DayTimelineMarker
  readonly selected: boolean
  readonly className: string
  readonly buttonRef: (button: HTMLButtonElement | null) => void
  readonly onSelect: (eventId: string) => void
  readonly onKeyDown: (event: KeyboardEvent<HTMLButtonElement>, eventId: string) => void
}

export function TimelineEventButton({
  marker,
  selected,
  className,
  buttonRef,
  onSelect,
  onKeyDown,
}: TimelineEventButtonProps) {
  return (
    <button
      aria-label={`${marker.dateKey} ${marker.timeLabel} ${categoryLabel(marker.category)}`}
      aria-pressed={selected}
      className={className}
      data-category={marker.category}
      data-day-event-id={marker.eventId}
      data-selected={selected}
      ref={buttonRef}
      type="button"
      onClick={() => onSelect(marker.eventId)}
      onKeyDown={(event) => onKeyDown(event, marker.eventId)}
    >
      <span className="day-timeline__marker-time">{marker.timeLabel}</span>
      <span className="day-timeline__marker-category">{categoryLabel(marker.category)}</span>
    </button>
  )
}
