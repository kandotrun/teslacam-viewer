import { useCallback, useRef, type Dispatch, type RefObject } from 'react'

import { buildTeslaCamLibrary } from '../teslacam/build-library'
import { filesFromDirectoryInput } from '../teslacam/directory-input'
import { pickTeslaCamDirectory } from '../teslacam/directory-picker'
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
  const isNativePickerAvailable = globalThis.showDirectoryPicker !== undefined

  const buildLibrary = useCallback(
    async (files: readonly LocalFileRef[], folderName: string): Promise<void> => {
      if (files.length === 0) {
        dispatch({ kind: 'scan-invalid' })
        return
      }
      try {
        const library = await buildTeslaCamLibrary(files, (progress) => {
          dispatch({ kind: 'scan-progressed', progress })
        })
        if (library.events.length === 0) {
          dispatch({ kind: 'scan-invalid' })
          return
        }
        dispatch({ kind: 'library-ready', folderName, library })
      } catch {
        dispatch({ kind: 'scan-failed' })
      }
    },
    [dispatch],
  )

  const selectNativeDirectory = useCallback(async (): Promise<void> => {
    dispatch({ kind: 'scan-started' })
    try {
      const selection = await pickTeslaCamDirectory((progress) => {
        dispatch({ kind: 'scan-progressed', progress })
      })
      switch (selection.status) {
        case 'selected':
          await buildLibrary(selection.files, selection.folderName)
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
      dispatch({ kind: 'scan-failed' })
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
      const selectedFiles = files === null ? [] : filesFromDirectoryInput(files)
      dispatch({ kind: 'scan-started' })
      void buildLibrary(selectedFiles, folderNameFromFiles(selectedFiles))
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
