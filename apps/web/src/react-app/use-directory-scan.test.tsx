import { act, renderHook } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, vi } from 'vitest'

import { MAX_DIRECTORY_DEPTH } from '../teslacam/scan-limits'
import type { AppAction } from './app-state'
import { useDirectoryScan } from './use-directory-scan'

type FakeDirectoryHandle = {
  readonly kind: 'directory'
  readonly name: string
  values: () => AsyncIterableIterator<FakeDirectoryHandle>
}

function nestedDirectory(depth: number): FakeDirectoryHandle {
  return {
    kind: 'directory',
    name: `depth-${depth}`,
    async *values() {
      if (depth > 0) yield nestedDirectory(depth - 1)
    },
  }
}

function fileWithPath(name: string, relativePath: string): File {
  const file = new File(['clip'], name, { type: 'video/mp4' })
  Object.defineProperty(file, 'webkitRelativePath', { value: relativePath })
  return file
}

async function fileList(files: readonly File[]): Promise<FileList> {
  const input = document.createElement('input')
  input.type = 'file'
  input.multiple = true
  await userEvent.upload(input, [...files])
  if (input.files === null) throw new Error('FileList unavailable')
  return input.files
}

afterEach(() => {
  Reflect.deleteProperty(window, 'showDirectoryPicker')
})

describe('useDirectoryScan generation', () => {
  it('ignores stale progress and completion from an earlier folder selection', async () => {
    let releaseSlowFile: () => void = () => undefined
    const slowFile = new Promise<File>((resolve) => {
      releaseSlowFile = () => resolve(new File(['clip'], '2026-08-20_20-45-00-front.mp4', { type: 'video/mp4' }))
    })
    Object.defineProperty(window, 'showDirectoryPicker', {
      configurable: true,
      value: async () => ({
        kind: 'directory',
        name: 'Slow',
        async *values() {
          yield { kind: 'file', name: '2026-08-20_20-45-00-front.mp4', getFile: () => slowFile }
        },
      }),
    })
    const currentFiles = await fileList([
      fileWithPath(
        '2026-08-21_14-30-45-front.mp4',
        'TeslaCam/RecentClips/2026-08-21_14-30-45-front.mp4',
      ),
    ])
    const actions: AppAction[] = []
    const hook = renderHook(() =>
      useDirectoryScan((action) => {
        actions.push(action)
      }),
    )

    act(() => hook.result.current.requestDirectory())
    await vi.waitFor(() => expect(actions.map((action) => action.kind)).toContain('scan-progressed'))
    act(() => hook.result.current.handleDirectoryInput(currentFiles))
    await vi.waitFor(() => expect(actions.at(-1)?.kind).toBe('library-ready'))
    releaseSlowFile()
    await act(async () => Promise.resolve())

    expect(actions.map((action) => action.kind)).toEqual([
      'scan-started',
      'scan-progressed',
      'scan-started',
      'scan-progressed',
      'library-ready',
    ])
    const ready = actions.at(-1)
    expect(ready?.kind === 'library-ready' ? ready.folderName : null).toBe('TeslaCam')
  })

  it('keeps the current library while the picker is open and when it is cancelled', async () => {
    Object.defineProperty(window, 'showDirectoryPicker', {
      configurable: true,
      value: async () => Promise.reject(new DOMException('', 'AbortError')),
    })
    const actions: AppAction[] = []
    const hook = renderHook(() =>
      useDirectoryScan((action) => {
        actions.push(action)
      }),
    )

    act(() => hook.result.current.requestDirectory())
    await vi.waitFor(() => expect(actions).toHaveLength(1))

    expect(actions.map((action) => action.kind)).toEqual(['scan-cancelled'])
  })

  it('cancels a running scan and ignores whatever it finds afterwards', async () => {
    let releaseFile: () => void = () => undefined
    const pendingFile = new Promise<File>((resolve) => {
      releaseFile = () => resolve(new File(['clip'], '2026-08-20_20-45-00-front.mp4', { type: 'video/mp4' }))
    })
    Object.defineProperty(window, 'showDirectoryPicker', {
      configurable: true,
      value: async () => ({
        kind: 'directory',
        name: 'RecentClips',
        async *values() {
          yield { kind: 'file', name: '2026-08-20_20-45-00-front.mp4', getFile: () => pendingFile }
        },
      }),
    })
    const actions: AppAction[] = []
    const hook = renderHook(() =>
      useDirectoryScan((action) => {
        actions.push(action)
      }),
    )

    act(() => hook.result.current.requestDirectory())
    await vi.waitFor(() => expect(actions.map((action) => action.kind)).toContain('scan-progressed'))
    act(() => hook.result.current.cancelScan())
    releaseFile()
    await act(async () => Promise.resolve())

    expect(actions.map((action) => action.kind)).toEqual(['scan-started', 'scan-progressed', 'scan-cancelled'])
  })

  it('surfaces a native scan limit violation through the safe failure action', async () => {
    Object.defineProperty(window, 'showDirectoryPicker', {
      configurable: true,
      value: async () => nestedDirectory(MAX_DIRECTORY_DEPTH + 1),
    })
    const actions: AppAction[] = []
    const hook = renderHook(() =>
      useDirectoryScan((action) => {
        actions.push(action)
      }),
    )

    act(() => hook.result.current.requestDirectory())
    await vi.waitFor(() => {
      expect(actions.at(-1)?.kind).toBe('scan-failed')
    })

    expect(actions.map((action) => action.kind)).toEqual([
      'scan-started',
      'scan-failed',
    ])
  })
})
