/**
 * Builders that turn compact fixture inputs into the web's real domain types.
 * The Mac test suite implements the same expansion rules (see packages/contract/README.md).
 */
import {
  cameraIds,
  type CameraId,
  type ClipCategory,
  type EventMetadata,
  type LocalFileRef,
  type TeslaCamEvent,
  type TeslaCamSegment,
} from '../teslacam/types'
import type { JsonValue } from './fixture-file'

/** A string input that expands to `repeat` concatenated `count` times. */
export type GeneratedString = { readonly $repeat: string; readonly count: number }

export function generated(repeat: string, count: number): GeneratedString {
  return { $repeat: repeat, count }
}

export function expandString(value: string | GeneratedString): string {
  return typeof value === 'string' ? value : value.$repeat.repeat(value.count)
}

/** Non-finite numbers are written as the strings `NaN`, `Infinity` and `-Infinity`. */
export function encodeNumber(value: number): number | string {
  if (Number.isNaN(value)) return 'NaN'
  if (value === Number.POSITIVE_INFINITY) return 'Infinity'
  if (value === Number.NEGATIVE_INFINITY) return '-Infinity'
  return value
}

/** Compact event used by fixture inputs: cameras are listed by id only. */
export type EventFixture = {
  readonly id: string
  readonly category: ClipCategory
  readonly timestamp: string
  readonly segments: readonly { readonly timestamp: string; readonly cameras: readonly CameraId[] }[]
  readonly metadata?: EventMetadata
  readonly hasRepresentative?: boolean
  readonly hasThumbnail?: boolean
}

type EventOptions = {
  readonly cameras?: readonly CameraId[]
  readonly segments?: readonly string[]
  readonly detection?: string
  readonly metadata?: EventMetadata
}

/** Shorthand for an event fixture whose segments all carry the same cameras. */
export function eventFixture(
  id: string,
  category: ClipCategory,
  timestamp: string,
  options: EventOptions = {},
): EventFixture {
  const cameras = options.cameras ?? cameraIds
  const metadata = options.metadata ?? (options.detection === undefined ? undefined : { timestamp: options.detection })
  return {
    id,
    category,
    timestamp,
    segments: (options.segments ?? [timestamp]).map((segment) => ({ timestamp: segment, cameras })),
    ...(metadata === undefined ? {} : { metadata }),
  }
}

export function eventFixtureJson(event: EventFixture): JsonValue {
  return {
    id: event.id,
    category: event.category,
    timestamp: event.timestamp,
    segments: event.segments.map((segment) => ({ timestamp: segment.timestamp, cameras: [...segment.cameras] })),
    ...(event.metadata === undefined ? {} : { metadata: { ...event.metadata } }),
    ...(event.hasRepresentative === true ? { hasRepresentative: true } : {}),
    ...(event.hasThumbnail === true ? { hasThumbnail: true } : {}),
  }
}

function placeholderFile(relativePath: string): LocalFileRef {
  const name = relativePath.split('/').at(-1) ?? relativePath
  return {
    name,
    relativePath,
    size: 1,
    type: 'video/mp4',
    getFile: async () => new File([new Uint8Array([1])], name),
  }
}

/** Builds a real `TeslaCamEvent` with placeholder file references. */
export function toTeslaCamEvent(event: EventFixture): TeslaCamEvent {
  const segments: TeslaCamSegment[] = event.segments.map((segment) => ({
    timestamp: segment.timestamp,
    cameras: Object.fromEntries(
      segment.cameras.map((camera) => [camera, placeholderFile(`${event.id}/${segment.timestamp}-${camera}.mp4`)]),
    ),
  }))
  return {
    id: event.id,
    category: event.category,
    timestamp: event.timestamp,
    segments,
    ...(event.hasRepresentative === true ? { representativeFile: placeholderFile(`${event.id}/event.mp4`) } : {}),
    ...(event.hasThumbnail === true ? { thumbnailFile: placeholderFile(`${event.id}/thumb.png`) } : {}),
    ...(event.metadata === undefined ? {} : { metadata: event.metadata }),
  }
}

export function metadataJson(metadata: EventMetadata): JsonValue {
  return { ...metadata }
}
