import type { ClipCategory, TeslaCamEvent } from '../teslacam/types'

export type EventFilters = {
  readonly category: ClipCategory | 'all'
  readonly dateKey: string | null
  readonly query: string
}

export function eventDate(timestamp: string): string {
  return timestamp.slice(0, 10)
}

export function eventClock(timestamp: string): string {
  return timestamp.slice(11).replaceAll('-', ':')
}

export function eventTime(timestamp: string): string {
  return `${eventDate(timestamp)} ${eventClock(timestamp)}`
}

export function categoryLabel(category: ClipCategory): string {
  switch (category) {
    case 'recent':
      return '最近の走行'
    case 'saved':
      return '保存済み'
    case 'sentry':
      return 'セントリー'
  }
}

export function eventMatchesFilters(event: TeslaCamEvent, filters: EventFilters): boolean {
  if (filters.category !== 'all' && event.category !== filters.category) return false
  if (filters.dateKey !== null && !event.timestamp.startsWith(filters.dateKey)) return false
  const query = filters.query.trim().toLowerCase()
  return (
    query === '' ||
    `${eventTime(event.timestamp)} ${event.category} ${categoryLabel(event.category)}`
      .toLowerCase()
      .includes(query)
  )
}

export function orderedMatchingEvents(
  events: readonly TeslaCamEvent[],
  filters: EventFilters,
): readonly TeslaCamEvent[] {
  return events
    .filter((event) => eventMatchesFilters(event, filters))
    .sort((left, right) => right.timestamp.localeCompare(left.timestamp))
}
