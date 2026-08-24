import { vi } from 'vitest'

import { createLocalFileRef } from '../test/teslacam-fixtures'
import { buildTeslaCamLibrary } from './build-library'
import { filesFromDirectoryInput } from './directory-input'
import { pickTeslaCamDirectory } from './directory-picker'
import {
  MAX_DIRECTORY_DEPTH,
  MAX_SCAN_ENTRIES,
  ScanLimitError,
} from './scan-limits'

type FakeDirectoryHandle = {
  readonly kind: 'directory'
  readonly name: string
  values: () => AsyncIterableIterator<FakeDirectoryHandle>
}

function nestedDirectory(depth: number): FakeDirectoryHandle {
  const name = `depth-${depth}`
  return {
    kind: 'directory',
    name,
    async *values() {
      if (depth > 0) yield nestedDirectory(depth - 1)
    },
  }
}

function selectDirectory(handle: FakeDirectoryHandle): void {
  Object.defineProperty(window, 'showDirectoryPicker', {
    configurable: true,
    value: async () => handle,
  })
}

afterEach(() => {
  Reflect.deleteProperty(window, 'showDirectoryPicker')
})

describe('local scan limits', () => {
  it('stops native traversal beyond the maximum recursion depth', async () => {
    selectDirectory(nestedDirectory(MAX_DIRECTORY_DEPTH + 1))

    const selection = pickTeslaCamDirectory(() => undefined)

    await expect(selection).rejects.toBeInstanceOf(ScanLimitError)
  })

  it('stops fallback ingestion before iterating too many entries', () => {
    const files = {
      length: MAX_SCAN_ENTRIES + 1,
      [Symbol.iterator](): Iterator<File> {
        return {
          next() {
            throw new Error('entry iteration must not start')
          },
        }
      },
    }

    expect(() => filesFromDirectoryInput(files)).toThrow(ScanLimitError)
  })

  it('stops fallback ingestion when a relative path exceeds the depth limit', () => {
    const file = new File(['clip'], 'clip.mp4')
    Object.defineProperty(file, 'webkitRelativePath', {
      value: `${Array.from({ length: MAX_DIRECTORY_DEPTH + 2 }, (_, index) => `d${index}`).join('/')}/clip.mp4`,
    })

    expect(() => filesFromDirectoryInput([file])).toThrow(ScanLimitError)
  })

  it('stops library construction when a caller bypasses ingestion limits', async () => {
    const files = Array.from(
      { length: MAX_SCAN_ENTRIES + 1 },
      (_, index) =>
        createLocalFileRef(`TeslaCam/RecentClips/unknown-${index}.txt`, {
          type: 'text/plain',
        }),
    )

    const library = buildTeslaCamLibrary(files, vi.fn())

    await expect(library).rejects.toBeInstanceOf(ScanLimitError)
  })

  it('rejects metadata whose declared file size exceeds one MiB', async () => {
    const clip = createLocalFileRef(
      'TeslaCam/SentryClips/event-a/2026-08-21_14-30-45-front.mp4',
    )
    const metadata = createLocalFileRef(
      'TeslaCam/SentryClips/event-a/event.json',
      { size: 1024 * 1024 + 1, type: 'application/json' },
    )

    const library = buildTeslaCamLibrary([clip, metadata], vi.fn())

    await expect(library).rejects.toBeInstanceOf(ScanLimitError)
  })

  it('rejects metadata whose File snapshot exceeds one MiB', async () => {
    const clip = createLocalFileRef(
      'TeslaCam/SentryClips/event-a/2026-08-21_14-30-45-front.mp4',
    )
    const snapshot = new File(
      [new Uint8Array(1024 * 1024 + 1)],
      'event.json',
      { type: 'application/json' },
    )
    const metadata = {
      name: snapshot.name,
      relativePath: 'TeslaCam/SentryClips/event-a/event.json',
      size: 1,
      type: snapshot.type,
      getFile: async () => snapshot,
    }

    const library = buildTeslaCamLibrary([clip, metadata], vi.fn())

    await expect(library).rejects.toBeInstanceOf(ScanLimitError)
  })
})
