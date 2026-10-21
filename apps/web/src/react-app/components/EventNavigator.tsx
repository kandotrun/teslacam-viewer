import type { ClipCategory, ScanWarning, TeslaCamEvent } from '../../teslacam/types'
import type { EventFilters } from '../event-filters'
import { EventList } from './EventList'
import { LibrarySidebar } from './LibrarySidebar'

type EventNavigatorProps = {
  readonly events: readonly TeslaCamEvent[]
  readonly selectedEventId: string | null
  readonly counts: Readonly<Record<ClipCategory, number>>
  readonly eventCount: number
  readonly filters: EventFilters
  readonly folderName: string
  readonly warnings: readonly ScanWarning[]
  readonly onCategory: (category: EventFilters['category']) => void
  readonly onDate: (dateKey: string | null) => void
  readonly onQuery: (query: string) => void
  readonly onSelectEvent: (eventId: string) => void
}

export function EventNavigator({
  events,
  selectedEventId,
  counts,
  eventCount,
  filters,
  folderName,
  warnings,
  onCategory,
  onDate,
  onQuery,
  onSelectEvent,
}: EventNavigatorProps) {
  return (
    <aside className="event-navigator" aria-label="イベントナビゲーション">
      <LibrarySidebar
        counts={counts}
        eventCount={eventCount}
        filters={filters}
        folderName={folderName}
        warnings={warnings}
        onCategory={onCategory}
        onDate={onDate}
        onQuery={onQuery}
      />
      <EventList
        events={events}
        selectedEventId={selectedEventId}
        onSelectEvent={onSelectEvent}
      />
    </aside>
  )
}
