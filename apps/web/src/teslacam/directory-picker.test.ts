import { afterEach, vi } from 'vitest'

import { buildTeslaCamLibrary } from './build-library'
import { pickTeslaCamDirectory } from './directory-picker'
import type { ScanProgress } from './types'

type FakeFileHandle = {
  readonly kind: 'file'
  readonly name: string
  getFile: () => Promise<File>
}

type FakeDirectoryHandle = {
  readonly kind: 'directory'
  readonly name: string
  values: () => AsyncIterableIterator<FakeHandle>
}

type FakeHandle = FakeFileHandle | FakeDirectoryHandle

function fileHandle(name: string, contents = 'clip'): FakeFileHandle {
  const file = new File([contents], name, { type: 'video/mp4' })
  return { kind: 'file', name, getFile: async () => file }
}

function directoryHandle(
  name: string,
  entries: readonly FakeHandle[],
): FakeDirectoryHandle {
  return {
    kind: 'directory',
    name,
    async *values() {
      yield* entries
    },
  }
}

function selectDirectory(handle: FakeDirectoryHandle): void {
  Object.defineProperty(window, 'showDirectoryPicker', {
    configurable: true,
    value: async () => handle,
  })
}

function rejectDirectory(name: string): void {
  Object.defineProperty(window, 'showDirectoryPicker', {
    configurable: true,
    value: async () => Promise.reject(new DOMException('', name)),
  })
}

afterEach(() => {
  vi.useRealTimers()
  Reflect.deleteProperty(window, 'showDirectoryPicker')
})

describe('pickTeslaCamDirectory', () => {
  it('recursively maps a selected TeslaCam root into Task 2 relative paths', async () => {
    const front = fileHandle('2026-08-21_14-30-45-front.mp4')
    const hidden = fileHandle('.DS_Store')
    const root = directoryHandle('TeslaCam', [
      directoryHandle('SavedClips', [
        directoryHandle('saved-a', [front, hidden]),
        directoryHandle('.ignored', [fileHandle('2026-08-21_14-30-45-back.mp4')]),
      ]),
    ])
    selectDirectory(root)

    const result = await pickTeslaCamDirectory(() => undefined)

    expect(result).toMatchObject({ status: 'selected', folderName: 'TeslaCam' })
    if (result.status === 'selected') {
      expect(result.files).toHaveLength(1)
      expect(result.files[0]?.relativePath).toBe(
        'TeslaCam/SavedClips/saved-a/2026-08-21_14-30-45-front.mp4',
      )
      expect(result.files[0]?.name).toBe('2026-08-21_14-30-45-front.mp4')
    }
  })

  it.each(['RecentClips', 'SavedClips', 'SentryClips'])(
    'prefixes a directly selected %s category so Task 2 recognizes it',
    async (category) => {
      const root = directoryHandle(category, [
        fileHandle('2026-08-21_14-30-45-front.mp4'),
      ])
      selectDirectory(root)

      const result = await pickTeslaCamDirectory(() => undefined)

      expect(result.status).toBe('selected')
      if (result.status === 'selected') {
        expect(result.files[0]?.relativePath).toBe(
          `${category}/2026-08-21_14-30-45-front.mp4`,
        )
      }
    },
  )

  it('preserves File snapshot metadata and identity from a scanned handle', async () => {
    const file = new File(['clip'], '2026-08-21_14-30-45-front.mp4', {
      type: 'video/x-tesla',
    })
    const lazyFile: FakeFileHandle = {
      kind: 'file',
      name: file.name,
      getFile: async () => file,
    }
    selectDirectory(directoryHandle('RecentClips', [lazyFile]))

    const result = await pickTeslaCamDirectory(() => undefined)

    expect(result.status).toBe('selected')
    if (result.status === 'selected') {
      expect(result.files[0]?.size).toBe(file.size)
      expect(result.files[0]?.type).toBe(file.type)
      await expect(result.files[0]?.getFile()).resolves.toBe(file)
    }
  })

  it('retains zero-byte File System Access clips for Task 2 empty-media warnings', async () => {
    const empty = new File([], '2026-08-21_14-30-45-front.mp4', {
      type: 'video/mp4',
    })
    const emptyHandle: FakeFileHandle = {
      kind: 'file',
      name: empty.name,
      getFile: async () => empty,
    }
    selectDirectory(directoryHandle('RecentClips', [emptyHandle]))

    const result = await pickTeslaCamDirectory(() => undefined)

    expect(result.status).toBe('selected')
    if (result.status === 'selected') {
      const library = await buildTeslaCamLibrary(result.files, () => undefined)

      expect(library.events).toEqual([])
      expect(library.warnings).toEqual([{ kind: 'empty-media-file', count: 1 }])
    }
  })

  it('reports and yields every 250 discovered entries before final progress', async () => {
    vi.useFakeTimers()
    const entries = Array.from({ length: 251 }, (_, index) =>
      fileHandle(`2026-08-21_14-30-45-front-${index}.mp4`),
    )
    selectDirectory(directoryHandle('RecentClips', entries))
    const progress: ScanProgress[] = []

    const selection = pickTeslaCamDirectory((update) => {
      progress.push(update)
    })
    for (let index = 0; index < 2000; index += 1) {
      await Promise.resolve()
    }

    expect(progress).toEqual([{ processed: 250, total: 250 }])
    await vi.advanceTimersByTimeAsync(0)
    await selection
    expect(progress).toEqual([
      { processed: 250, total: 250 },
      { processed: 251, total: 251 },
    ])
  })

  it.each([
    ['AbortError', 'cancelled'],
    ['NotAllowedError', 'denied'],
  ])('returns %s selection status for %s', async (errorName, status) => {
    rejectDirectory(errorName)

    const result = await pickTeslaCamDirectory(() => undefined)

    expect(result).toEqual({ status })
  })

  it('returns unsupported when the directory picker is unavailable', async () => {
    const result = await pickTeslaCamDirectory(() => undefined)

    expect(result).toEqual({ status: 'unsupported' })
  })
})
