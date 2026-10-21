import { useCallback, useRef, type Dispatch, type RefObject } from 'react'

import { buildTeslaCamLibrary } from '../teslacam/build-library'
import { filesFromDirectoryInput } from '../teslacam/directory-input'
import { pickTeslaCamDirectory } from '../teslacam/directory-picker'
import { ScanLimitError } from '../teslacam/scan-limits'
import type { LocalFileRef } from '../teslacam/types'
import type { AppAction } from './app-state'

type DirectoryScan = {
  readonly fallbackInputRef: RefObject<HTMLInputElement | null>
  readonly isNativePickerAvailable: boolean
  readonly requestDirectory: () => void
  readonly handleDirectoryInput: (files: FileList | null) => void
}

function folderNameFromFiles(files: readonly LocalFileRef[]): string {
  const firstFile = files.at(0)
  if (firstFile === undefined) return 'TeslaCam'
  return firstFile.relativePath.split('/').at(0) ?? 'TeslaCam'
}

export function useDirectoryScan(dispatch: Dispatch<AppAction>): DirectoryScan {
  const fallbackInputRef = useRef<HTMLInputElement>(null)
  const scanGenerationRef = useRef(0)
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

  const selectNativeDirectory = useCallback(async (): Promise<void> => {
    scanGenerationRef.current += 1
    const generation = scanGenerationRef.current
    dispatch({ kind: 'scan-started' })
    try {
      const selection = await pickTeslaCamDirectory((progress) => {
        if (scanGenerationRef.current === generation) {
          dispatch({ kind: 'scan-progressed', progress })
        }
      })
      if (scanGenerationRef.current !== generation) return
      switch (selection.status) {
        case 'selected':
          await buildLibrary(selection.files, selection.folderName, generation)
          return
        case 'cancelled':
          dispatch({ kind: 'scan-cancelled' })
          return
        case 'denied':
          dispatch({ kind: 'scan-denied' })
          return
        case 'unsupported':
          dispatch({ kind: 'scan-cancelled' })
          fallbackInputRef.current?.click()
          return
      }
    } catch {
      if (scanGenerationRef.current === generation) {
        dispatch({ kind: 'scan-failed' })
      }
    }
  }, [buildLibrary, dispatch])

  const requestDirectory = useCallback((): void => {
    if (globalThis.showDirectoryPicker === undefined) {
      fallbackInputRef.current?.click()
      return
    }
    void selectNativeDirectory()
  }, [selectNativeDirectory])

  const handleDirectoryInput = useCallback(
    (files: FileList | null): void => {
      scanGenerationRef.current += 1
      const generation = scanGenerationRef.current
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
    [buildLibrary, dispatch],
  )

  return {
    fallbackInputRef,
    isNativePickerAvailable,
    requestDirectory,
    handleDirectoryInput,
  }
}
