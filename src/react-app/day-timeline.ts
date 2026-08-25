import type { ClipCategory, TeslaCamEvent } from '../teslacam/types'

import { parseFilenameLocalTimestamp, type LocalTimestamp } from './local-timestamp'

const finalSecondOfDay = 86_399
const desktopTimelineTargetWidth = 44

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

export type TimelineDay = {
  readonly dateKey: string
  readonly eventIds: readonly string[]
}

export type TimelineDayAdjacency = {
  readonly previous: TimelineDay | undefined
  readonly next: TimelineDay | undefined
}

export type DayTimelineCluster = {
  readonly id: string
  readonly count: number
  readonly markers: readonly DayTimelineMarker[]
  readonly startTimeLabel: string
  readonly endTimeLabel: string
  readonly startSecondsOfDay: number
  readonly endSecondsOfDay: number
  readonly durationSeconds: number
  readonly startPositionPercent: number
  readonly endPositionPercent: number
}

function eventTimestamp(event: TeslaCamEvent): string {
  if (event.category !== 'recent') return event.timestamp
  return event.segments[0]?.timestamp ?? event.timestamp
}

function eventLocalTimestamp(event: TeslaCamEvent): LocalTimestamp | null {
  return parseFilenameLocalTimestamp(eventTimestamp(event))
}

function timelineMarker(event: TeslaCamEvent): DayTimelineMarker | null {
  const timestamp = eventLocalTimestamp(event)
  if (timestamp === null) return null

  return {
    eventId: event.id,
    category: event.category,
    dateKey: timestamp.dateKey,
    timeLabel: timestamp.timeLabel,
    secondsOfDay: timestamp.secondsOfDay,
    positionPercent: timestamp.secondsOfDay / finalSecondOfDay * 100,
  }
}

function compareTimelineMarkers(left: DayTimelineMarker, right: DayTimelineMarker): number {
  const secondsDifference = left.secondsOfDay - right.secondsOfDay
  if (secondsDifference !== 0) return secondsDifference
  const categoryDifference = categoryOrder[left.category] - categoryOrder[right.category]
  if (categoryDifference !== 0) return categoryDifference
  return left.eventId.localeCompare(right.eventId)
}

function createTimelineCluster(
  markers: readonly DayTimelineMarker[],
): DayTimelineCluster | undefined {
  const startMarker = markers[0]
  const endMarker = markers.at(-1)
  if (startMarker === undefined || endMarker === undefined) return undefined

  return {
    id: startMarker.eventId,
    count: markers.length,
    markers,
    startTimeLabel: startMarker.timeLabel,
    endTimeLabel: endMarker.timeLabel,
    startSecondsOfDay: startMarker.secondsOfDay,
    endSecondsOfDay: endMarker.secondsOfDay,
    durationSeconds: endMarker.secondsOfDay - startMarker.secondsOfDay,
    startPositionPercent: startMarker.positionPercent,
    endPositionPercent: endMarker.positionPercent,
  }
}

function targetAreasOverlap(
  previous: DayTimelineMarker,
  next: DayTimelineMarker,
  trackWidthPixels: number,
): boolean {
  if (!Number.isFinite(trackWidthPixels) || trackWidthPixels <= 0) return false
  const distancePixels = (next.positionPercent - previous.positionPercent) / 100 * trackWidthPixels
  return distancePixels < desktopTimelineTargetWidth
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

  const markers: DayTimelineMarker[] = []
  for (const event of events) {
    const marker = timelineMarker(event)
    if (marker !== null && marker.dateKey === selectedDate) markers.push(marker)
  }
  return markers.sort(compareTimelineMarkers)
}

export function buildTimelineDays(events: readonly TeslaCamEvent[]): readonly TimelineDay[] {
  const markersByDate = new Map<string, DayTimelineMarker[]>()
  for (const event of events) {
    const marker = timelineMarker(event)
    if (marker === null) continue

    const dayMarkers = markersByDate.get(marker.dateKey)
    if (dayMarkers === undefined) {
      markersByDate.set(marker.dateKey, [marker])
      continue
    }
    dayMarkers.push(marker)
  }

  return Array.from(markersByDate.entries())
    .sort(([leftDateKey], [rightDateKey]) => leftDateKey.localeCompare(rightDateKey))
    .map(([dateKey, markers]) => ({
      dateKey,
      eventIds: markers.sort(compareTimelineMarkers).map((marker) => marker.eventId),
    }))
}

export function adjacentTimelineDays(
  days: readonly TimelineDay[],
  currentDateKey: string | null,
): TimelineDayAdjacency {
  const currentDate = parsedDateKey(currentDateKey)
  if (currentDate === null) return { previous: undefined, next: undefined }

  let previous: TimelineDay | undefined
  let next: TimelineDay | undefined
  for (const day of days) {
    if (day.dateKey < currentDate && (previous === undefined || day.dateKey > previous.dateKey)) {
      previous = day
    }
    if (day.dateKey > currentDate && (next === undefined || day.dateKey < next.dateKey)) {
      next = day
    }
  }
  return { previous, next }
}

export function buildDayTimelineClusters(
  events: readonly TeslaCamEvent[],
  dateKey: string | null,
  trackWidthPixels: number,
): readonly DayTimelineCluster[] {
  const markers = buildDayTimelineMarkers(events, dateKey)
  const clusters: DayTimelineCluster[] = []
  let clusterMarkers: DayTimelineMarker[] = []
  let previousMarker: DayTimelineMarker | undefined

  for (const marker of markers) {
    if (
      previousMarker !== undefined
      && targetAreasOverlap(previousMarker, marker, trackWidthPixels)
    ) {
      clusterMarkers.push(marker)
      previousMarker = marker
      continue
    }

    const cluster = createTimelineCluster(clusterMarkers)
    if (cluster !== undefined) clusters.push(cluster)
    clusterMarkers = [marker]
    previousMarker = marker
  }

  const finalCluster = createTimelineCluster(clusterMarkers)
  if (finalCluster !== undefined) clusters.push(finalCluster)
  return clusters
}
