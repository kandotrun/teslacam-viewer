import { useReducer, useRef, type RefObject } from 'react'

import { buildTeslaCamLibrary } from '../teslacam/build-library'
import { filesFromDirectoryInput } from '../teslacam/directory-input'
import { pickTeslaCamDirectory } from '../teslacam/directory-picker'
import type { LocalFileRef } from '../teslacam/types'
import { appReducer, initialAppState } from './app-state'
import { AppHeader } from './components/AppHeader'
import { EmptyState } from './components/EmptyState'
import { EventList } from './components/EventList'
import { LibrarySidebar } from './components/LibrarySidebar'
import { ScanProgress } from './components/ScanProgress'

function folderNameFromFiles(files: readonly LocalFileRef[]): string {
  const firstFile = files.at(0)
  if (firstFile === undefined) return 'TeslaCam'
  return firstFile.relativePath.split('/').at(0) ?? 'TeslaCam'
}

export function App() {
  const [state, dispatch] = useReducer(appReducer, initialAppState)
  const fallbackInputRef = useRef<HTMLInputElement>(null)

  const buildLibrary = async (
    files: readonly LocalFileRef[],
    folderName: string,
  ): Promise<void> => {
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
    } catch (error) {
      if (error instanceof Error || error instanceof DOMException) {
        dispatch({ kind: 'scan-failed' })
        return
      }
      throw error
    }
  }

  const selectDirectory = async (): Promise<void> => {
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
    } catch (error) {
      if (error instanceof Error || error instanceof DOMException) {
        dispatch({ kind: 'scan-failed' })
        return
      }
      throw error
    }
  }

  const changeDirectory = (files: FileList | null): void => {
    const selectedFiles = files === null ? [] : filesFromDirectoryInput(files)
    dispatch({ kind: 'scan-started' })
    void buildLibrary(selectedFiles, folderNameFromFiles(selectedFiles))
  }

  switch (state.kind) {
    case 'idle':
      return (
        <>
          <EmptyState error={null} onSelect={() => void selectDirectory()} />
          {directoryInput(fallbackInputRef, changeDirectory)}
        </>
      )
    case 'scanning':
      return (
        <>
          <ScanProgress progress={state.progress} />
          {directoryInput(fallbackInputRef, changeDirectory)}
        </>
      )
    case 'error':
      return (
        <>
          <EmptyState error={state.reason} onSelect={() => void selectDirectory()} />
          {directoryInput(fallbackInputRef, changeDirectory)}
        </>
      )
    case 'ready': {
      const selectedEvent = state.library.events.find(
        (event) => event.id === state.selectedEventId,
      )
      const counts = {
        recent: state.library.eventsByCategory.recent.length,
        saved: state.library.eventsByCategory.saved.length,
        sentry: state.library.eventsByCategory.sentry.length,
      }
      return (
        <div className="library-shell">
          <AppHeader onRescan={() => void selectDirectory()} />
          <main className="library-workspace">
            <LibrarySidebar
              counts={counts}
              filters={state.filters}
              warnings={state.library.warnings}
              onCategory={(category) => dispatch({ kind: 'category-filtered', category })}
              onDate={(dateKey) => dispatch({ kind: 'date-filtered', dateKey })}
              onQuery={(query) => dispatch({ kind: 'query-filtered', query })}
            />
            <EventList
              events={state.library.events}
              filters={state.filters}
              selectedEventId={state.selectedEventId}
              onSelectEvent={(eventId) => dispatch({ kind: 'event-selected', eventId })}
            />
            <section className="selection-placeholder" aria-labelledby="selection-title">
              <p className="sidebar-label">THEATER</p>
              <h1 id="selection-title">
                {selectedEvent === undefined
                  ? 'イベントを選択してください'
                  : '選択したイベントを準備しています'}
              </h1>
              <p>
                {selectedEvent === undefined
                  ? '一覧からイベントを選ぶと、次の段階で6方向の映像をここに表示します。'
                  : `${selectedEvent.segments.length} セグメントを選択しました。再生は次の段階で利用できます。`}
              </p>
            </section>
          </main>
          {directoryInput(fallbackInputRef, changeDirectory)}
        </div>
      )
    }
  }
}

function directoryInput(
  ref: RefObject<HTMLInputElement | null>,
  onChange: (files: FileList | null) => void,
) {
  return (
    <input
      aria-label="TeslaCamフォルダを選ぶ（互換モード）"
      className="directory-input"
      ref={ref}
      type="file"
      webkitdirectory=""
      onClick={(event) => {
        event.currentTarget.value = ''
      }}
      onChange={(event) => onChange(event.currentTarget.files)}
    />
  )
}
