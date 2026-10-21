/**
 * What the library panel shows for a ready app state: the counts, the day on screen, its rows and rail
 * markers, the neighbouring recorded days and the older/newer events for the transport. Pure.
 */
import type { ClipCategory, TeslaCamEvent } from '../../teslacam/types'
import type { AppAction, ReadyState } from '../app-state'
import {
  adjacentTimelineDays,
  buildDayTimelineMarkers,
  buildTimelineDays,
  selectTimelineDate,
  type DayTimelineMarker,
  type TimelineDay,
} from '../day-timeline'
import { orderedMatchingEvents } from '../event-filters'

export type CategoryCounts = Readonly<Record<'all' | ClipCategory, number>>

export type LibraryView = {
  readonly counts: CategoryCounts
  /** Category and query apply, the day does not (parity spec §1.15). Newest first. */
  readonly timelineEvents: readonly TeslaCamEvent[]
  /** The day on screen: the selected event's day, else the date filter, else the newest recorded day. */
  readonly dayKey: string | null
  /** Rows of the day on screen, newest first. */
  readonly dayEvents: readonly TeslaCamEvent[]
  readonly markers: readonly DayTimelineMarker[]
  readonly previousDay: TimelineDay | undefined
  readonly nextDay: TimelineDay | undefined
  readonly selectedEvent: TeslaCamEvent | undefined
  /** `過去の記録` target. */
  readonly olderEventId: string | null
  /** `新しい記録` target. */
  readonly newerEventId: string | null
}

export function eventDateKey(event: TeslaCamEvent): string {
  return event.timestamp.slice(0, 10)
}

export function buildLibraryView(state: ReadyState): LibraryView {
  const { library, filters, selectedEventId } = state
  const timelineEvents = orderedMatchingEvents(library.events, { ...filters, dateKey: null })
  const selectedEvent = library.events.find((event) => event.id === selectedEventId)
  const dayKey = selectTimelineDate(timelineEvents, selectedEvent, filters.dateKey)
  const dayEvents = dayKey === null ? [] : orderedMatchingEvents(library.events, { ...filters, dateKey: dayKey })
  const adjacent = adjacentTimelineDays(buildTimelineDays(timelineEvents), dayKey)
  const selectedIndex = timelineEvents.findIndex((event) => event.id === selectedEventId)
  return {
    counts: {
      all: library.events.length,
      recent: library.eventsByCategory.recent.length,
      saved: library.eventsByCategory.saved.length,
      sentry: library.eventsByCategory.sentry.length,
    },
    timelineEvents,
    dayKey,
    dayEvents,
    markers: buildDayTimelineMarkers(timelineEvents, dayKey),
    previousDay: adjacent.previous,
    nextDay: adjacent.next,
    selectedEvent,
    olderEventId: selectedIndex < 0 ? null : (timelineEvents[selectedIndex + 1]?.id ?? null),
    newerEventId: selectedIndex <= 0 ? null : (timelineEvents[selectedIndex - 1]?.id ?? null),
  }
}

/** Selecting an event keeps an active day filter in step with the event's day. */
export function selectEventActions(state: ReadyState, eventId: string): readonly AppAction[] {
  const event = state.library.events.find((candidate) => candidate.id === eventId)
  if (event === undefined) return []
  const dateKey = eventDateKey(event)
  const select: AppAction = { kind: 'event-selected', eventId }
  return state.filters.dateKey === null || state.filters.dateKey === dateKey
    ? [select]
    : [{ kind: 'date-filtered', dateKey }, select]
}

/** Moving to a recorded day selects (and so plays) its newest event (parity spec §1.20.2). */
export function selectDayActions(day: TimelineDay): readonly AppAction[] {
  const newest = day.eventIds.at(-1)
  const filter: AppAction = { kind: 'date-filtered', dateKey: day.dateKey }
  return newest === undefined ? [filter] : [filter, { kind: 'event-selected', eventId: newest }]
}

/** A date chosen in the picker; days without recordings show an empty day. */
export function pickDateActions(view: LibraryView, dateKey: string | null): readonly AppAction[] {
  if (dateKey === null) return [{ kind: 'date-filtered', dateKey: null }]
  const day = buildTimelineDays(view.timelineEvents).find((candidate) => candidate.dateKey === dateKey)
  return day === undefined ? [{ kind: 'date-filtered', dateKey }] : selectDayActions(day)
}
