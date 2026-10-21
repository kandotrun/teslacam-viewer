import { useCallback, useRef, type Dispatch, type RefObject } from 'react'

import { buildTeslaCamLibrary } from '../teslacam/build-library'
import { filesFromDirectoryInput } from '../teslacam/directory-input'
import {
  requestTeslaCamDirectory,
  scanTeslaCamDirectory,
  type DirectoryRequestResult,
} from '../teslacam/directory-picker'
import { ScanLimitError } from '../teslacam/scan-limits'
import type { LocalFileRef } from '../teslacam/types'
import type { AppAction } from './app-state'

type DirectoryScan = {
  readonly fallbackInputRef: RefObject<HTMLInputElement | null>
  readonly isNativePickerAvailable: boolean
  readonly requestDirectory: () => void
  readonly handleDirectoryInput: (files: FileList | null) => void
  /** Stops a running scan; its late progress and result are ignored. */
  readonly cancelScan: () => void
}

function folderNameFromFiles(files: readonly LocalFileRef[]): string {
  const firstFile = files.at(0)
  if (firstFile === undefined) return 'TeslaCam'
  return firstFile.relativePath.split('/').at(0) ?? 'TeslaCam'
}

export function useDirectoryScan(dispatch: Dispatch<AppAction>): DirectoryScan {
  const fallbackInputRef = useRef<HTMLInputElement>(null)
  const scanGenerationRef = useRef(0)
  const scanAbortRef = useRef<AbortController | null>(null)
  /** Set while `showDirectoryPicker()` is open; a second concurrent call would be rejected as denied. */
  const pickerPendingRef = useRef(false)
  const isNativePickerAvailable = globalThis.showDirectoryPicker !== undefined

  const buildLibrary = useCallback(
    async (
      files: readonly LocalFileRef[],
      folderName: string,
      generation: number,
    ): Promise<void> => {
      const isCurrent = (): boolean => scanGenerationRef.current === generation
      if (files.length === 0) {
        if (isCurrent()) dispatch({ kind: 'scan-invalid' })
        return
      }
      try {
        const library = await buildTeslaCamLibrary(files, (progress) => {
          if (isCurrent()) dispatch({ kind: 'scan-progressed', progress })
        })
        if (!isCurrent()) return
        if (library.events.length === 0) {
          dispatch({ kind: 'scan-invalid' })
          return
        }
        dispatch({ kind: 'library-ready', folderName, library })
      } catch {
        if (isCurrent()) dispatch({ kind: 'scan-failed' })
      }
    },
    [dispatch],
  )

  const nextGeneration = useCallback((): number => {
    scanAbortRef.current?.abort()
    scanAbortRef.current = null
    scanGenerationRef.current += 1
    return scanGenerationRef.current
  }, [])

  const selectNativeDirectory = useCallback(async (): Promise<void> => {
    if (pickerPendingRef.current) return
    const generation = nextGeneration()
    const isCurrent = (): boolean => scanGenerationRef.current === generation
    try {
      // The loaded library stays on screen while the picker is open; a cancelled re-pick keeps it.
      pickerPendingRef.current = true
      let request: DirectoryRequestResult
      try {
        request = await requestTeslaCamDirectory()
      } finally {
        pickerPendingRef.current = false
      }
      if (!isCurrent()) return
      switch (request.status) {
        case 'cancelled':
          dispatch({ kind: 'scan-cancelled' })
          return
        case 'denied':
          dispatch({ kind: 'scan-denied' })
          return
        case 'unsupported':
          fallbackInputRef.current?.click()
          return
        case 'picked':
          break
      }
      const controller = new AbortController()
      scanAbortRef.current = controller
      dispatch({ kind: 'scan-started' })
      const selection = await scanTeslaCamDirectory(request.root, (progress) => {
        if (isCurrent()) dispatch({ kind: 'scan-progressed', progress })
      }, { signal: controller.signal })
      if (!isCurrent()) return
      await buildLibrary(selection.files, selection.folderName, generation)
    } catch {
      if (isCurrent()) dispatch({ kind: 'scan-failed' })
    }
  }, [buildLibrary, dispatch, nextGeneration])

  const cancelScan = useCallback((): void => {
    nextGeneration()
    dispatch({ kind: 'scan-cancelled' })
  }, [dispatch, nextGeneration])

  const requestDirectory = useCallback((): void => {
    if (globalThis.showDirectoryPicker === undefined) {
      fallbackInputRef.current?.click()
      return
    }
    void selectNativeDirectory()
  }, [selectNativeDirectory])

  const handleDirectoryInput = useCallback(
    (files: FileList | null): void => {
      const generation = nextGeneration()
      dispatch({ kind: 'scan-started' })
      let selectedFiles: readonly LocalFileRef[]
      try {
        selectedFiles = files === null ? [] : filesFromDirectoryInput(files)
      } catch (error) {
        if (error instanceof ScanLimitError) {
          dispatch({ kind: 'scan-failed' })
          return
        }
        throw error
      }
      void buildLibrary(
        selectedFiles,
        folderNameFromFiles(selectedFiles),
        generation,
      )
    },
    [buildLibrary, dispatch, nextGeneration],
  )

  return {
    fallbackInputRef,
    isNativePickerAvailable,
    requestDirectory,
    handleDirectoryInput,
    cancelScan,
  }
}
