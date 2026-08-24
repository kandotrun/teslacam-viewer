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
  vi.useRealTimers()
  Reflect.deleteProperty(window, 'showDirectoryPicker')
})

describe('useDirectoryScan generation', () => {
  it('ignores stale progress and completion from an earlier folder selection', async () => {
    const slowFiles = await fileList(
      Array.from({ length: 501 }, (_, index) =>
        fileWithPath(
          `unknown-${index}.txt`,
          `TeslaCam/RecentClips/unknown-${index}.txt`,
        ),
      ),
    )
    const currentFiles = await fileList([
      fileWithPath(
        '2026-08-21_14-30-45-front.mp4',
        'TeslaCam/RecentClips/2026-08-21_14-30-45-front.mp4',
      ),
    ])
    vi.useFakeTimers()
    const actions: AppAction[] = []
    const hook = renderHook(() =>
      useDirectoryScan((action) => {
        actions.push(action)
      }),
    )

    act(() => hook.result.current.handleDirectoryInput(slowFiles))
    act(() => hook.result.current.handleDirectoryInput(currentFiles))
    await act(async () => Promise.resolve())
    await act(async () => vi.runAllTimersAsync())

    expect(actions.map((action) => action.kind)).toEqual([
      'scan-started',
      'scan-progressed',
      'scan-started',
      'scan-progressed',
      'library-ready',
    ])
    expect(
      actions.flatMap((action) =>
        action.kind === 'scan-progressed' ? [action.progress.processed] : [],
      ),
    ).toEqual([250, 1])
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
