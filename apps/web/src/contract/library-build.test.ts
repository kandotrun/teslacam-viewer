// @vitest-environment node
import { describe } from 'vitest'

import { buildTeslaCamLibrary } from '../teslacam/build-library'
import { MAX_EVENT_METADATA_SIZE, MAX_METADATA_STRING_LENGTH, MAX_SCAN_ENTRIES, ScanLimitError } from '../teslacam/scan-limits'
import { cameraIds, type LocalFileRef, type ScanProgress, type TeslaCamEvent } from '../teslacam/types'
import { expandString, generated, metadataJson, type GeneratedString } from './adapters'
import { defineFixture, type FixtureCase, type JsonValue } from './fixture-file'

/**
 * One collected file. `name` defaults to the last `/` component of `relativePath`; `size` defaults to the
 * UTF-8 byte length of `text`, else 14 (the length of the default contents `synthetic-file`).
 */
type InputFile = {
  readonly relativePath: string
  readonly name?: string
  readonly size?: number
  readonly text?: string | GeneratedString
}

type LibraryInput = {
  readonly files: readonly InputFile[]
  /** Appends `count` files at `pathPattern` (`{i}` = 0-based index) after `files`. */
  readonly generate?: { readonly count: number; readonly pathPattern: string; readonly size?: number }
}

const defaultContents = 'synthetic-file'

function toFileRef(input: InputFile): LocalFileRef {
  const name = input.name ?? input.relativePath.split('/').at(-1) ?? input.relativePath
  const bytes = new TextEncoder().encode(input.text === undefined ? defaultContents : expandString(input.text))
  const file = new File([bytes], name)
  return { name, relativePath: input.relativePath, size: input.size ?? bytes.length, type: '', getFile: async () => file }
}

function* inputFiles(input: LibraryInput): Generator<InputFile> {
  yield* input.files
  const generate = input.generate
  if (generate === undefined) return
  for (let index = 0; index < generate.count; index += 1) {
    yield {
      relativePath: generate.pathPattern.replaceAll('{i}', String(index)),
      ...(generate.size === undefined ? {} : { size: generate.size }),
    }
  }
}

function inputFileJson(input: InputFile): JsonValue {
  return {
    relativePath: input.relativePath,
    ...(input.name === undefined ? {} : { name: input.name }),
    ...(input.size === undefined ? {} : { size: input.size }),
    ...(input.text === undefined
      ? {}
      : { text: typeof input.text === 'string' ? input.text : { $repeat: input.text.$repeat, count: input.text.count } }),
  }
}

function eventJson(event: TeslaCamEvent): JsonValue {
  return {
    id: event.id,
    category: event.category,
    timestamp: event.timestamp,
    segments: event.segments.map((segment) => ({
      timestamp: segment.timestamp,
      cameras: Object.fromEntries(
        cameraIds.flatMap((camera) => {
          const file = segment.cameras[camera]
          return file === undefined ? [] : [[camera, file.relativePath]]
        }),
      ),
    })),
    representative: event.representativeFile?.relativePath ?? null,
    thumbnail: event.thumbnailFile?.relativePath ?? null,
    metadata: event.metadata === undefined ? null : metadataJson(event.metadata),
  }
}

async function libraryCase(name: string, input: LibraryInput, note?: string): Promise<FixtureCase> {
  const files = [...inputFiles(input)].map(toFileRef)
  const progress: ScanProgress[] = []
  let expected: JsonValue
  try {
    const library = await buildTeslaCamLibrary(files, (update) => {
      progress.push(update)
    })
    expected = {
      events: library.events.map(eventJson),
      eventsByCategory: {
        recent: library.eventsByCategory.recent.map((event) => event.id),
        saved: library.eventsByCategory.saved.map((event) => event.id),
        sentry: library.eventsByCategory.sentry.map((event) => event.id),
      },
      warnings: library.warnings.map((warning) => ({ kind: warning.kind, count: warning.count })),
      progress: progress.map((update) => ({ processed: update.processed, total: update.total })),
    }
  } catch (error) {
    if (!(error instanceof ScanLimitError)) throw error
    expected = { error: 'scan-limit', kind: error.kind }
  }
  return {
    name,
    fn: 'buildTeslaCamLibrary',
    ...(note === undefined ? {} : { note }),
    input: {
      files: input.files.map(inputFileJson),
      ...(input.generate === undefined
        ? {}
        : {
            generate: {
              count: input.generate.count,
              pathPattern: input.generate.pathPattern,
              ...(input.generate.size === undefined ? {} : { size: input.generate.size }),
            },
          }),
    },
    expected,
  }
}

function clip(directory: string, timestamp: string, camera: string, size?: number): InputFile {
  return { relativePath: `${directory}/${timestamp}-${camera}.mp4`, ...(size === undefined ? {} : { size }) }
}

function sixCameras(directory: string, timestamp: string): InputFile[] {
  return cameraIds.map((camera) => clip(directory, timestamp, camera))
}

const sentryJson = JSON.stringify({
  timestamp: '2026-08-20T20:35:30',
  reason: 'sentry_aware_object_detection',
  city: 'SYNTHETIC_PRIVATE_CITY',
  street: 'SYNTHETIC_PRIVATE_STREET',
  est_lat: 'SYNTHETIC_PRIVATE_LAT',
  est_lon: 'SYNTHETIC_PRIVATE_LON',
}, null, 2)

describe('contract: library-build', () => {
  defineFixture({
    file: 'library-build',
    module: 'library-build',
    source: [
      'apps/web/src/teslacam/build-library.ts',
      'apps/web/src/teslacam/event-metadata.ts',
      'apps/web/src/teslacam/filename.ts',
      'apps/web/src/teslacam/scan-limits.ts',
    ],
    cases: async () => [
      await libraryCase('saved clips grouped by directory and timestamp', {
        files: [
          clip('TeslaCam/SavedClips/saved-a', '2026-08-21_14-30-45', 'front'),
          clip('TeslaCam/SavedClips/saved-a', '2026-08-21_14-30-45', 'back'),
          clip('TeslaCam/SavedClips/saved-a', '2026-08-21_14-31-45', 'front'),
          { relativePath: 'TeslaCam/SavedClips/saved-a/event.mp4' },
          { relativePath: 'TeslaCam/SavedClips/saved-a/thumb.png' },
          {
            relativePath: 'TeslaCam/SavedClips/saved-a/event.json',
            text: JSON.stringify({ timestamp: '2026-08-21_14-30-45', reason: 'manual_save', ignored: 'untrusted' }),
          },
          clip('TeslaCam/SavedClips/saved-b', '2026-08-21_15-30-45', 'left_pillar'),
        ],
      }),
      await libraryCase('six cameras in one segment', { files: sixCameras('TeslaCam/SavedClips/saved-six', '2026-08-21_16-30-45') }),
      await libraryCase('sentry directories with equal timestamps', {
        files: [
          clip('TeslaCam/SentryClips/sentry-a', '2026-08-21_14-30-45', 'right_repeater'),
          clip('TeslaCam/SentryClips/sentry-b', '2026-08-21_14-30-45', 'right_repeater'),
        ],
      }),
      await libraryCase('excluded files keep sanitized counts', {
        files: [
          clip('TeslaCam/SavedClips/saved-a', '2026-08-21_14-30-45', 'front', 0),
          clip('TeslaCam/SavedClips/saved-a', '2026-08-21_14-30-45', 'cabin'),
          { relativePath: 'TeslaCam/SavedClips/saved-a/unrecognized.bin' },
          { relativePath: 'TeslaCam/EncryptedClips/2026-08-21_14-30-45-front.mp4' },
        ],
      }),
      await libraryCase('recent sessions across midnight', {
        files: [
          clip('TeslaCam/RecentClips', '2026-08-21_23-59-30', 'front'),
          clip('TeslaCam/RecentClips', '2026-08-22_00-00-50', 'back'),
          clip('TeslaCam/RecentClips', '2026-08-22_00-02-21', 'front'),
        ],
      }),
      await libraryCase('recent gap of exactly 90 seconds joins', {
        files: [
          clip('TeslaCam/RecentClips', '2026-08-21_10-00-00', 'front'),
          clip('TeslaCam/RecentClips', '2026-08-21_10-01-30', 'front'),
          clip('TeslaCam/RecentClips', '2026-08-21_10-03-01', 'front'),
        ],
      }),
      await libraryCase('recent session across a year boundary', {
        files: [
          clip('TeslaCam/RecentClips', '2027-01-01_00-00-30', 'front'),
          clip('TeslaCam/RecentClips', '2026-12-31_23-59-30', 'back'),
          clip('TeslaCam/RecentClips', '2026-12-31_23-58-30', 'left_pillar'),
        ],
      }),
      await libraryCase('recent session across a leap day', {
        files: [
          clip('TeslaCam/RecentClips', '2028-02-28_23-59-00', 'front'),
          clip('TeslaCam/RecentClips', '2028-02-29_00-00-00', 'front'),
          clip('TeslaCam/RecentClips', '2028-03-01_00-00-00', 'front'),
        ],
      }),
      await libraryCase('recent clips in sub-directories are accepted', {
        files: [
          clip('TeslaCam/RecentClips/nested/deeper', '2026-08-21_10-00-00', 'front'),
          clip('TeslaCam/RecentClips', '2026-08-21_10-01-00', 'back'),
        ],
      }),
      await libraryCase('helpers inside RecentClips are unknown files', {
        files: [
          clip('TeslaCam/RecentClips', '2026-08-21_10-00-00', 'front'),
          { relativePath: 'TeslaCam/RecentClips/event.mp4' },
          { relativePath: 'TeslaCam/RecentClips/thumb.png' },
          { relativePath: 'TeslaCam/RecentClips/event.json', text: '{"timestamp": "2026-08-21T10:00:10"}' },
        ],
      }),
      await libraryCase('a later helper replaces an earlier one', {
        files: [
          { relativePath: 'TeslaCam/SavedClips/a/event.mp4' },
          { relativePath: 'TeslaCam/SavedClips/a/nested/event.mp4' },
          { relativePath: 'TeslaCam/SavedClips/a/thumb.png', size: 0 },
          { relativePath: 'TeslaCam/SavedClips/a/event.json', text: '{"timestamp": "first"}' },
          { relativePath: 'TeslaCam/SavedClips/a/nested/event.json', text: '{"timestamp": "second"}' },
          clip('TeslaCam/SavedClips/a/nested', '2026-08-21_10-00-00', 'front'),
        ],
      }),
      await libraryCase('a zero-byte event.mp4 is an empty media file', {
        files: [
          { relativePath: 'TeslaCam/SentryClips/a/event.mp4', size: 0 },
          { relativePath: 'TeslaCam/SentryClips/a/event.json', size: 0, text: '' },
          clip('TeslaCam/SentryClips/a', '2026-08-21_10-00-00', 'front'),
        ],
      }),
      await libraryCase('a clip directly inside SavedClips forms its own event', {
        files: [
          clip('TeslaCam/SavedClips', '2026-08-21_10-00-00', 'front'),
          clip('TeslaCam/SavedClips', '2026-08-21_10-00-00', 'back'),
          { relativePath: 'TeslaCam/SavedClips/event.mp4' },
        ],
      }),
      await libraryCase('a selected SentryClips root', {
        files: [clip('SentryClips/event-a', '2026-08-21_10-00-00', 'front'), clip('SentryClips', '2026-08-21_11-00-00', 'front')],
      }),
      await libraryCase('duplicate timestamp and camera: the later file wins', {
        files: [
          clip('TeslaCam/SavedClips/a', '2026-08-21_10-00-00', 'front'),
          clip('TeslaCam/SavedClips/a/sub', '2026-08-21_10-00-00', 'front'),
          clip('TeslaCam/RecentClips', '2026-08-21_10-00-00', 'front'),
          clip('TeslaCam/RecentClips/sub', '2026-08-21_10-00-00', 'front'),
        ],
      }),
      await libraryCase('the first category name in the path wins', {
        files: [
          clip('TeslaCam/EncryptedClips/SavedClips/a', '2026-08-21_10-00-00', 'front'),
          clip('TeslaCam/SavedClips/EncryptedClips', '2026-08-21_10-00-00', 'front'),
          clip('TeslaCam/SavedClips/SentryClips', '2026-08-21_11-00-00', 'front'),
          clip('TeslaCam/RecentClips/SavedClips/a', '2026-08-21_12-00-00', 'front'),
          clip('SentryClips/RecentClips', '2026-08-21_13-00-00', 'front'),
        ],
      }),
      await libraryCase('backslash separators are accepted', {
        files: [
          { relativePath: 'TeslaCam\\SavedClips\\a\\2026-08-21_10-00-00-front.mp4', name: '2026-08-21_10-00-00-front.mp4' },
          { relativePath: 'TeslaCam/SavedClips\\a/event.mp4', name: 'event.mp4' },
        ],
      }),
      await libraryCase('files outside every category', {
        files: [
          { relativePath: 'TeslaCam/notes.txt' },
          { relativePath: 'TeslaCam/2026-08-21_10-00-00-front.mp4', size: 0 },
          { relativePath: 'TeslaCam/2026-08-21_10-00-00-front.MP4', size: 0 },
          { relativePath: 'TeslaCam/2026-08-21_10-00-00-front.mp4' },
          { relativePath: 'TeslaCam/Recentclips/2026-08-21_10-00-00-front.mp4' },
        ],
      }),
      await libraryCase('encrypted files are counted whatever their name or size', {
        files: [
          { relativePath: 'TeslaCam/EncryptedClips/a.bin', size: 0 },
          { relativePath: 'TeslaCam/EncryptedClips/event.json', size: 0 },
          { relativePath: 'TeslaCam/EncryptedClips/2026-08-21_10-00-00-front.mp4', size: 0 },
        ],
      }),
      await libraryCase('an upper-case zero-byte clip is unknown', {
        files: [{ relativePath: 'TeslaCam/RecentClips/2026-08-21_10-00-00-front.MP4', size: 0 }],
      }),
      await libraryCase('an event with only helpers is dropped without reading metadata', {
        files: [
          { relativePath: 'TeslaCam/SentryClips/only-helpers/event.json', text: 'not json' },
          { relativePath: 'TeslaCam/SentryClips/only-helpers/event.mp4' },
          { relativePath: 'TeslaCam/SentryClips/only-helpers/thumb.png' },
          { relativePath: 'TeslaCam/SentryClips/oversized/event.json', size: MAX_EVENT_METADATA_SIZE + 1 },
          clip('TeslaCam/SentryClips/kept', '2026-08-21_10-00-00', 'front'),
        ],
      }),
      await libraryCase('invalid event.json adds a warning and no metadata', {
        files: [
          clip('TeslaCam/SentryClips/a', '2026-08-21_10-00-00', 'front'),
          { relativePath: 'TeslaCam/SentryClips/a/event.json', text: '{"timestamp": 42}' },
          clip('TeslaCam/SentryClips/b', '2026-08-21_11-00-00', 'front'),
          { relativePath: 'TeslaCam/SentryClips/b/event.json', text: 'null' },
          clip('TeslaCam/SavedClips/c', '2026-08-21_12-00-00', 'front'),
          { relativePath: 'TeslaCam/SavedClips/c/event.json', text: '{"timestamp": "2026-08-21T12:00:10", "reason": 42}' },
        ],
      }),
      await libraryCase('event timestamp comes from the first segment, not event.json', {
        files: [
          clip('TeslaCam/SentryClips/a', '2026-08-21_10-01-00', 'front'),
          clip('TeslaCam/SentryClips/a', '2026-08-21_10-00-00', 'back'),
          { relativePath: 'TeslaCam/SentryClips/a/event.json', text: '{"timestamp": "2026-08-21T10:01:30", "camera": "5"}' },
        ],
      }),
      await libraryCase('directory events are never split by gaps', {
        files: [
          clip('TeslaCam/SavedClips/a', '2026-08-21_10-00-00', 'front'),
          clip('TeslaCam/SavedClips/a', '2026-08-22_10-00-00', 'front'),
        ],
      }),
      await libraryCase('equal timestamps across and within categories', {
        files: [
          clip('TeslaCam/SentryClips/z', '2026-08-21_10-00-00', 'front'),
          clip('TeslaCam/SavedClips/y', '2026-08-21_10-00-00', 'front'),
          clip('TeslaCam/RecentClips', '2026-08-21_10-00-00', 'front'),
          clip('TeslaCam/SentryClips/a', '2026-08-21_10-00-00', 'back'),
          clip('TeslaCam/SavedClips/x', '2026-08-21_09-00-00', 'front'),
          clip('TeslaCam/SavedClips/b', '2026-08-21_10-00-00', 'back'),
        ],
      }),
      await libraryCase('metadata string limit aborts the scan', {
        files: [
          clip('TeslaCam/SentryClips/a', '2026-08-21_10-00-00', 'front'),
          {
            relativePath: 'TeslaCam/SentryClips/a/event.json',
            text: JSON.stringify({ timestamp: '2026-08-21T10:00:10', street: 'x'.repeat(MAX_METADATA_STRING_LENGTH + 1) }),
          },
        ],
      }),
      await libraryCase('metadata at the string limit is kept', {
        files: [
          clip('TeslaCam/SentryClips/a', '2026-08-21_10-00-00', 'front'),
          {
            relativePath: 'TeslaCam/SentryClips/a/event.json',
            text: JSON.stringify({ timestamp: '2026-08-21T10:00:10', street: 'x'.repeat(MAX_METADATA_STRING_LENGTH) }),
          },
        ],
      }),
      await libraryCase('declared metadata size over one MiB aborts the scan', {
        files: [
          clip('TeslaCam/SentryClips/event-a', '2026-08-21_14-30-45', 'front'),
          { relativePath: 'TeslaCam/SentryClips/event-a/event.json', size: MAX_EVENT_METADATA_SIZE + 1 },
        ],
      }),
      await libraryCase('actual metadata bytes over one MiB abort the scan', {
        files: [
          clip('TeslaCam/SentryClips/event-a', '2026-08-21_14-30-45', 'front'),
          { relativePath: 'TeslaCam/SentryClips/event-a/event.json', size: 1, text: generated(' ', MAX_EVENT_METADATA_SIZE + 1) },
        ],
      }),
      await libraryCase('no files', { files: [] }),
      await libraryCase('exactly 250 files report once', {
        files: [],
        generate: { count: 250, pathPattern: 'TeslaCam/RecentClips/unknown-{i}.txt' },
      }),
      await libraryCase('251 unknown files', {
        files: [],
        generate: { count: 251, pathPattern: 'TeslaCam/RecentClips/unknown-{i}.txt' },
      }),
      await libraryCase('600 files report every 250', {
        files: [clip('TeslaCam/RecentClips', '2026-08-21_10-00-00', 'front')],
        generate: { count: 599, pathPattern: 'TeslaCam/Noise/n-{i}.txt', size: 1 },
      }),
      await libraryCase(`${MAX_SCAN_ENTRIES + 1} files exceed the entry limit`, {
        files: [],
        generate: { count: MAX_SCAN_ENTRIES + 1, pathPattern: 'TeslaCam/RecentClips/unknown-{i}.txt' },
      }),
      await libraryCase('the e2e fixture directory', {
        files: [
          { relativePath: 'TeslaCam/SentryClips/event-sentry/sentry-autoplay-20s.mp4', size: 39_210 },
          { relativePath: 'TeslaCam/SentryClips/event-sentry/event.json', text: sentryJson },
          { relativePath: 'TeslaCam/RecentClips/2026-08-20_20-45-00-front.mp4', size: 4_096 },
        ],
      }, 'apps/web/tests/e2e/fixtures/TeslaCam listed in name order'),
      await libraryCase('the e2e full tree', {
        files: [
          ...sixCameras('TeslaCam/RecentClips', '2026-08-20_20-45-00'),
          ...sixCameras('TeslaCam/SavedClips/event-saved', '2026-08-20_20-40-00'),
          { relativePath: 'TeslaCam/SavedClips/event-saved/event.mp4' },
          ...sixCameras('TeslaCam/SentryClips/event-sentry', '2026-08-20_20-35-00'),
          { relativePath: 'TeslaCam/SentryClips/event-sentry/event.json', text: sentryJson },
        ],
        generate: { count: 253, pathPattern: 'TeslaCam/Noise/synthetic-{i}.txt', size: 1 },
      }, 'tests/e2e/support/viewer-fixtures.ts variant full'),
      await libraryCase('the e2e history tree', {
        files: [
          ...sixCameras('TeslaCam/RecentClips', '2026-08-20_20-45-00'),
          ...sixCameras('TeslaCam/SavedClips/event-saved', '2026-08-20_20-40-00'),
          { relativePath: 'TeslaCam/SavedClips/event-saved/event.mp4' },
          ...sixCameras('TeslaCam/SavedClips/event-saved-previous', '2026-08-19_19-30-00'),
          { relativePath: 'TeslaCam/SavedClips/event-saved-previous/event.mp4' },
          ...sixCameras('TeslaCam/SentryClips/event-sentry', '2026-08-20_20-35-00').slice(0, 5),
          { relativePath: 'TeslaCam/SentryClips/event-sentry/event.json', text: sentryJson },
        ],
      }, 'variant history with the Sentry right repeater missing (variant missing)'),
      await libraryCase('the e2e export tree', {
        files: ['20-10-00', '20-10-02', '20-10-04', '20-10-06', '20-10-08'].flatMap((time) =>
          sixCameras('TeslaCam/SavedClips/event-export', `2026-08-20_${time}`)),
      }),
      await libraryCase('the e2e wrong folder', { files: [{ relativePath: 'SyntheticFolder/notes.txt', size: 1 }] }),
    ],
  })
})
