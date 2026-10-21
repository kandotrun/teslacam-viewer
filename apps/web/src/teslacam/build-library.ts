import { createYieldBudget } from './cooperative-yield'
import { readEventMetadata } from './event-metadata'
import { parseClipFileName } from './filename'
import { ensureScanEntryCount } from './scan-limits'
import { mapWithConcurrency } from './task-pool'
import type { CameraId, ClipCategory, LocalFileRef, ScanProgress, ScanWarning, TeslaCamEvent, TeslaCamLibrary, TeslaCamSegment } from './types'

type DirectoryEvent = {
  category: Exclude<ClipCategory, 'recent'>
  clips: Map<string, Map<CameraId, LocalFileRef>>
  representativeFile?: LocalFileRef
  thumbnailFile?: LocalFileRef
  metadataFile?: LocalFileRef
}

type FileLocation = { readonly kind: 'clip'; readonly category: ClipCategory; readonly parts: readonly string[] } | { readonly kind: 'encrypted' } | { readonly kind: 'unknown' }

const warningKinds = [
  'empty-media-file',
  'invalid-metadata-file',
  'unknown-file',
  'encrypted-file',
] as const

function categoryFromPath(relativePath: string): FileLocation {
  const parts = relativePath.split(/[\\/]/u)
  for (const part of parts) {
    switch (part) {
      case 'RecentClips':
        return { kind: 'clip', category: 'recent', parts }
      case 'SavedClips':
        return { kind: 'clip', category: 'saved', parts }
      case 'SentryClips':
        return { kind: 'clip', category: 'sentry', parts }
      case 'EncryptedClips':
        return { kind: 'encrypted' }
      default:
        break
    }
  }
  return { kind: 'unknown' }
}

function directoryKey(
  category: Exclude<ClipCategory, 'recent'>,
  parts: readonly string[],
): string | null {
  const categoryDirectory = category === 'saved' ? 'SavedClips' : 'SentryClips'
  const categoryIndex = parts.indexOf(categoryDirectory)
  const parent = parts.at(categoryIndex + 1)
  return categoryIndex < 0 || parent === undefined ? null : `${category}:${parent}`
}

function isHelperName(name: string): boolean {
  return name === 'event.mp4' || name === 'event.json' || name === 'thumb.png'
}

function daysBeforeMonth(year: number, month: number): number {
  const baseDays = [0, 0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334] as const
  const base = baseDays[month] ?? 0
  return base + (month > 2 && isLeapYear(year) ? 1 : 0)
}

function isLeapYear(year: number): boolean {
  return year % 4 === 0 && (year % 100 !== 0 || year % 400 === 0)
}

function timestampValue(timestamp: string): number {
  const match = /^(\d{4})-(\d{2})-(\d{2})_(\d{2})-(\d{2})-(\d{2})$/u.exec(
    timestamp,
  )
  if (match === null) return 0

  const year = Number(match[1])
  const month = Number(match[2])
  const day = Number(match[3])
  const hour = Number(match[4])
  const minute = Number(match[5])
  const second = Number(match[6])
  const previousYear = year - 1
  const days =
    previousYear * 365 +
    Math.floor(previousYear / 4) -
    Math.floor(previousYear / 100) +
    Math.floor(previousYear / 400) +
    daysBeforeMonth(year, month) +
    day -
    1

  return (((days * 24 + hour) * 60 + minute) * 60 + second)
}

function createSegments(
  clips: Map<string, Map<CameraId, LocalFileRef>>,
): readonly TeslaCamSegment[] {
  return [...clips.entries()]
    .sort(([left], [right]) => timestampValue(left) - timestampValue(right))
    .map(([timestamp, cameras]) => ({
      timestamp,
      cameras: Object.fromEntries(cameras),
    }))
}

/** `event.json` files read at once; results are still applied in folder order. */
const metadataReadConcurrency = 4

function eventSort(left: TeslaCamEvent, right: TeslaCamEvent): number {
  return timestampValue(right.timestamp) - timestampValue(left.timestamp)
}

function warningList(counts: ReadonlyMap<ScanWarning['kind'], number>): readonly ScanWarning[] {
  return warningKinds.flatMap((kind) => {
    const count = counts.get(kind)
    return count === undefined ? [] : [{ kind, count }]
  })
}

function createDirectoryEvent(
  category: Exclude<ClipCategory, 'recent'>,
): DirectoryEvent {
  return { category, clips: new Map() }
}

export async function buildTeslaCamLibrary(
  files: readonly LocalFileRef[],
  report: (progress: ScanProgress) => void,
): Promise<TeslaCamLibrary> {
  ensureScanEntryCount(files.length)
  const budget = createYieldBudget()
  const directoryEvents = new Map<string, DirectoryEvent>()
  const recentClips = new Map<string, Map<CameraId, LocalFileRef>>()
  const warningCounts = new Map<ScanWarning['kind'], number>()
  const addWarning = (kind: ScanWarning['kind']): void => {
    warningCounts.set(kind, (warningCounts.get(kind) ?? 0) + 1)
  }

  for (const [index, file] of files.entries()) {
    const location = categoryFromPath(file.relativePath)
    if (location.kind === 'encrypted') {
      addWarning('encrypted-file')
    } else if (location.kind === 'unknown') {
      addWarning(file.name.endsWith('.mp4') && file.size === 0 ? 'empty-media-file' : 'unknown-file')
    } else if (file.name.endsWith('.mp4') && file.size === 0) {
      addWarning('empty-media-file')
    } else if (location.category !== 'recent' && isHelperName(file.name)) {
      const key = directoryKey(location.category, location.parts)
      if (key === null) {
        addWarning('unknown-file')
      } else {
        const event =
          directoryEvents.get(key) ?? createDirectoryEvent(location.category)
        if (file.name === 'event.mp4') event.representativeFile = file
        if (file.name === 'thumb.png') event.thumbnailFile = file
        if (file.name === 'event.json') event.metadataFile = file
        directoryEvents.set(key, event)
      }
    } else {
      const parsed = parseClipFileName(file.name)
      if (parsed === null) {
        addWarning('unknown-file')
      } else if (location.category === 'recent') {
        const cameras = recentClips.get(parsed.timestamp) ?? new Map()
        cameras.set(parsed.camera, file)
        recentClips.set(parsed.timestamp, cameras)
      } else {
        const key = directoryKey(location.category, location.parts)
        if (key === null) {
          addWarning('unknown-file')
        } else {
          const event =
            directoryEvents.get(key) ?? createDirectoryEvent(location.category)
          const cameras = event.clips.get(parsed.timestamp) ?? new Map()
          cameras.set(parsed.camera, file)
          event.clips.set(parsed.timestamp, cameras)
          directoryEvents.set(key, event)
        }
      }
    }

    const processed = index + 1
    if (processed % 250 === 0) report({ processed, total: files.length })
    await budget.checkpoint()
  }

  if (files.length % 250 !== 0) report({ processed: files.length, total: files.length })

  const eventsWithSegments = [...directoryEvents.values()].flatMap((directoryEvent) => {
    const segments = createSegments(directoryEvent.clips)
    const firstSegment = segments.at(0)
    return firstSegment === undefined ? [] : [{ directoryEvent, segments, firstSegment }]
  })
  const metadataOutcomes = await mapWithConcurrency(
    eventsWithSegments,
    metadataReadConcurrency,
    async ({ directoryEvent }) => directoryEvent.metadataFile === undefined
      ? undefined
      : readEventMetadata(directoryEvent.metadataFile),
  )

  const savedEvents: TeslaCamEvent[] = []
  const sentryEvents: TeslaCamEvent[] = []
  for (const [index, { directoryEvent, segments, firstSegment }] of eventsWithSegments.entries()) {
    const outcome = metadataOutcomes[index]
    if (outcome === undefined) throw new Error('Event metadata outcome is missing')
    if (outcome.status === 'rejected') throw outcome.reason
    const metadata = outcome.value
    if (directoryEvent.metadataFile !== undefined && metadata === null) {
      addWarning('invalid-metadata-file')
    }

    const event = {
      id: '',
      category: directoryEvent.category,
      timestamp: firstSegment.timestamp,
      segments,
      ...(directoryEvent.representativeFile === undefined
        ? {}
        : { representativeFile: directoryEvent.representativeFile }),
      ...(directoryEvent.thumbnailFile === undefined
        ? {}
        : { thumbnailFile: directoryEvent.thumbnailFile }),
      ...(metadata === undefined || metadata === null ? {} : { metadata }),
    }
    if (directoryEvent.category === 'saved') savedEvents.push(event)
    if (directoryEvent.category === 'sentry') sentryEvents.push(event)
  }

  const recentEvents: TeslaCamEvent[] = []
  const recentSegments = createSegments(recentClips)
  let session: TeslaCamSegment[] = []
  for (const segment of recentSegments) {
    const previous = session.at(-1)
    if (
      previous !== undefined &&
      timestampValue(segment.timestamp) - timestampValue(previous.timestamp) > 90
    ) {
      recentEvents.push({
        id: '',
        category: 'recent',
        timestamp: session[0]?.timestamp ?? segment.timestamp,
        segments: session,
      })
      session = []
    }
    session.push(segment)
  }
  const firstSessionSegment = session.at(0)
  if (firstSessionSegment !== undefined) {
    recentEvents.push({
      id: '',
      category: 'recent',
      timestamp: firstSessionSegment.timestamp,
      segments: session,
    })
  }

  const indexedEvents = (events: TeslaCamEvent[]): readonly TeslaCamEvent[] =>
    events
      .sort(eventSort)
      .map((event, index) => ({
        ...event,
        id: `${event.category}-${event.timestamp}-${index + 1}`,
      }))
  const eventsByCategory = {
    recent: indexedEvents(recentEvents),
    saved: indexedEvents(savedEvents),
    sentry: indexedEvents(sentryEvents),
  }
  const events = [
    ...eventsByCategory.recent,
    ...eventsByCategory.saved,
    ...eventsByCategory.sentry,
  ].sort(eventSort)

  return { events, eventsByCategory, warnings: warningList(warningCounts) }
}
