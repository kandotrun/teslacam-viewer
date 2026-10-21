import { afterEach, vi } from 'vitest'

import { buildTeslaCamLibrary } from './build-library'
import {
  DIRECTORY_SNAPSHOT_CONCURRENCY,
  pickTeslaCamDirectory,
  requestTeslaCamDirectory,
  scanTeslaCamDirectory,
} from './directory-picker'
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

  it('reports progress every 250 discovered entries and once more at the end', async () => {
    const entries = Array.from({ length: 251 }, (_, index) =>
      fileHandle(`2026-08-21_14-30-45-front-${index}.mp4`),
    )
    selectDirectory(directoryHandle('RecentClips', entries))
    const progress: ScanProgress[] = []

    await pickTeslaCamDirectory((update) => {
      progress.push(update)
    })

    expect(progress).toEqual([
      { processed: 250, total: 250 },
      { processed: 251, total: 251 },
    ])
  })

  it('walks the tree without waiting for each file snapshot and reads snapshots in parallel', async () => {
    let inFlight = 0
    let peak = 0
    const release: Array<() => void> = []
    let enumerated = 0
    const files = Array.from({ length: 40 }, (_, index): FakeFileHandle => {
      const name = `2026-08-21_14-30-${String(index).padStart(2, '0')}-front.mp4`
      return {
        kind: 'file',
        name,
        getFile: () => {
          inFlight += 1
          peak = Math.max(peak, inFlight)
          return new Promise<File>((resolve) => {
            release.push(() => {
              inFlight -= 1
              resolve(new File(['clip'], name))
            })
          })
        },
      }
    })
    const root: FakeDirectoryHandle = {
      kind: 'directory',
      name: 'RecentClips',
      async *values() {
        for (const file of files) {
          enumerated += 1
          yield file
        }
      },
    }
    selectDirectory(root)

    const selection = pickTeslaCamDirectory(() => undefined)
    await vi.waitFor(() => expect(enumerated).toBe(40))
    expect(peak).toBeGreaterThan(1)
    while (release.length > 0) {
      release.shift()?.()
      await Promise.resolve()
    }
    const result = await selection

    expect(peak).toBeLessThanOrEqual(DIRECTORY_SNAPSHOT_CONCURRENCY)
    expect(result.status).toBe('selected')
    if (result.status === 'selected') {
      expect(result.files.map((file) => file.name)).toEqual(files.map((file) => file.name))
      expect(result.files.every((file) => file.size === 4)).toBe(true)
    }
  })

  it('gets the playable File from the handle lazily and only once', async () => {
    const file = new File(['clip'], '2026-08-21_14-30-45-front.mp4', { type: 'video/mp4' })
    const getFile = vi.fn(async () => file)
    selectDirectory(directoryHandle('RecentClips', [{ kind: 'file', name: file.name, getFile }]))

    const result = await pickTeslaCamDirectory(() => undefined)
    if (result.status !== 'selected') throw new Error('directory was not selected')
    const scannedCalls = getFile.mock.calls.length
    const ref = result.files[0]
    if (ref === undefined) throw new Error('file reference is missing')

    await expect(ref.getFile()).resolves.toBe(file)
    await expect(ref.getFile()).resolves.toBe(file)
    expect(getFile.mock.calls.length).toBe(scannedCalls + 1)
  })

  it('separates choosing a folder from scanning it', async () => {
    const root = directoryHandle('RecentClips', [fileHandle('2026-08-21_14-30-45-front.mp4')])
    selectDirectory(root)

    const request = await requestTeslaCamDirectory()
    expect(request.status).toBe('picked')
    if (request.status !== 'picked') return
    const result = await scanTeslaCamDirectory(request.root, () => undefined)

    expect(result).toMatchObject({ status: 'selected', folderName: 'RecentClips' })
  })

  it('stops walking when the scan is aborted', async () => {
    const controller = new AbortController()
    const root: FakeDirectoryHandle = {
      kind: 'directory',
      name: 'RecentClips',
      async *values() {
        yield fileHandle('2026-08-21_14-30-45-front.mp4')
        controller.abort()
        yield fileHandle('2026-08-21_14-30-45-back.mp4')
      },
    }

    await expect(scanTeslaCamDirectory(root, () => undefined, { signal: controller.signal }))
      .rejects.toMatchObject({ name: 'AbortError' })
  })

  it('stops the queued size reads when aborted after the walk has finished', async () => {
    const controller = new AbortController()
    const fileCount = DIRECTORY_SNAPSHOT_CONCURRENCY * 3
    let started = 0
    const never = new Promise<File>(() => undefined)
    const entries = Array.from({ length: fileCount }, (_, index): FakeFileHandle => ({
      kind: 'file',
      name: `2026-08-21_14-${String(index).padStart(2, '0')}-00-front.mp4`,
      getFile: () => {
        started += 1
        return never
      },
    }))

    const scan = scanTeslaCamDirectory(directoryHandle('RecentClips', entries), (progress) => {
      // The final report is sent once the walk is done and only the size reads remain.
      if (progress.processed === fileCount) controller.abort()
    }, { signal: controller.signal })

    await expect(scan).rejects.toMatchObject({ name: 'AbortError' })
    expect(started).toBe(DIRECTORY_SNAPSHOT_CONCURRENCY)
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
