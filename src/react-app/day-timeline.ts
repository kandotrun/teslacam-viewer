import type { ClipCategory, TeslaCamEvent } from '../teslacam/types'

import { parseFilenameLocalTimestamp, type LocalTimestamp } from './local-timestamp'

const finalSecondOfDay = 86_399

const categoryOrder: Readonly<Record<ClipCategory, number>> = {
  sentry: 0,
  saved: 1,
  recent: 2,
}

export type DayTimelineMarker = {
  readonly eventId: string
  readonly category: ClipCategory
  readonly dateKey: string
  readonly timeLabel: string
  readonly secondsOfDay: number
  readonly positionPercent: number
}

function eventTimestamp(event: TeslaCamEvent): string {
  if (event.category !== 'recent') return event.timestamp
  return event.segments[0]?.timestamp ?? event.timestamp
}

function eventLocalTimestamp(event: TeslaCamEvent): LocalTimestamp | null {
  return parseFilenameLocalTimestamp(eventTimestamp(event))
}

function parsedDateKey(dateKey: string | null): string | null {
  if (dateKey === null) return null
  return parseFilenameLocalTimestamp(`${dateKey}_00-00-00`)?.dateKey ?? null
}

export function selectTimelineDate(
  events: readonly TeslaCamEvent[],
  selectedEvent: TeslaCamEvent | undefined,
  filterDateKey: string | null,
): string | null {
  const selectedDate = selectedEvent === undefined ? null : eventLocalTimestamp(selectedEvent)?.dateKey
  if (selectedDate !== null && selectedDate !== undefined) return selectedDate

  const filteredDate = parsedDateKey(filterDateKey)
  if (filteredDate !== null) return filteredDate

  for (const event of events) {
    const eventDate = eventLocalTimestamp(event)?.dateKey
    if (eventDate !== undefined) return eventDate
  }
  return null
}

export function buildDayTimelineMarkers(
  events: readonly TeslaCamEvent[],
  dateKey: string | null,
): readonly DayTimelineMarker[] {
  const selectedDate = parsedDateKey(dateKey)
  if (selectedDate === null) return []

  return events.flatMap((event) => {
    const timestamp = eventLocalTimestamp(event)
    if (timestamp === null || timestamp.dateKey !== selectedDate) return []

    return [{
      eventId: event.id,
      category: event.category,
      dateKey: timestamp.dateKey,
      timeLabel: timestamp.timeLabel,
      secondsOfDay: timestamp.secondsOfDay,
      positionPercent: timestamp.secondsOfDay / finalSecondOfDay * 100,
    }]
  }).sort((left, right) => {
    const secondsDifference = left.secondsOfDay - right.secondsOfDay
    if (secondsDifference !== 0) return secondsDifference
    const categoryDifference = categoryOrder[left.category] - categoryOrder[right.category]
    if (categoryDifference !== 0) return categoryDifference
    return left.eventId.localeCompare(right.eventId)
  })
}
