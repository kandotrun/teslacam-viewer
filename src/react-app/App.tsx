import { useReducer, useRef, type RefObject } from 'react'

import { buildTeslaCamLibrary } from '../teslacam/build-library'
import { filesFromDirectoryInput } from '../teslacam/directory-input'
import { pickTeslaCamDirectory } from '../teslacam/directory-picker'
import type { LocalFileRef } from '../teslacam/types'
import { appReducer, initialAppState, orderedMatchingEvents } from './app-state'
import { AppHeader } from './components/AppHeader'
import { EmptyState } from './components/EmptyState'
import { EventList } from './components/EventList'
import { LibrarySidebar } from './components/LibrarySidebar'
import { ScanProgress } from './components/ScanProgress'
import { Theater } from './components/Theater'

function folderNameFromFiles(files: readonly LocalFileRef[]): string {
  const firstFile = files.at(0)
  if (firstFile === undefined) return 'TeslaCam'
  return firstFile.relativePath.split('/').at(0) ?? 'TeslaCam'
}

export function App() {
  const [state, dispatch] = useReducer(appReducer, initialAppState)
  const fallbackInputRef = useRef<HTMLInputElement>(null)
  const theaterRef = useRef<HTMLElement>(null)
  const nativePickerAvailable = globalThis.showDirectoryPicker !== undefined

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
    } catch {
      dispatch({ kind: 'scan-failed' })
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
    } catch {
      dispatch({ kind: 'scan-failed' })
    }
  }

  const requestDirectory = (): void => {
    if (!nativePickerAvailable) {
      fallbackInputRef.current?.click()
      return
    }
    void selectDirectory()
  }

  const changeDirectory = (files: FileList | null): void => {
    const selectedFiles = files === null ? [] : filesFromDirectoryInput(files)
    dispatch({ kind: 'scan-started' })
    void buildLibrary(selectedFiles, folderNameFromFiles(selectedFiles))
  }

  const selectEvent = (eventId: string): void => {
    dispatch({ kind: 'event-selected', eventId })
    if (!globalThis.matchMedia('(max-width: 1024px)').matches) return
    requestAnimationFrame(() => theaterRef.current?.scrollIntoView({ block: 'start' }))
  }

  switch (state.kind) {
    case 'idle':
      return (
        <>
          <EmptyState error={null} onSelect={requestDirectory} />
          {!nativePickerAvailable && compatibilityStatus()}
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
          <EmptyState error={state.reason} onSelect={requestDirectory} />
          {!nativePickerAvailable && compatibilityStatus()}
          {directoryInput(fallbackInputRef, changeDirectory)}
        </>
      )
    case 'ready': {
      const selectedEvent = state.library.events.find(
        (event) => event.id === state.selectedEventId,
      )
      const matchingEvents = orderedMatchingEvents(state.library.events, state.filters)
      const selectedIndex = matchingEvents.findIndex((event) => event.id === state.selectedEventId)
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
              onSelectEvent={selectEvent}
            />
            {selectedEvent === undefined ? (
              <section className="selection-placeholder" aria-labelledby="selection-title">
                <p className="sidebar-label">THEATER</p>
                <h1 id="selection-title">イベントを選択してください</h1>
                <p className="selection-description">
                  <span>一覧からイベントを選ぶと、</span>
                  <span>6方向の映像をここに表示します。</span>
                </p>
              </section>
            ) : (
              <Theater
                event={selectedEvent}
                hasNext={selectedIndex >= 0 && selectedIndex < matchingEvents.length - 1}
                hasPrevious={selectedIndex > 0}
                key={selectedEvent.id}
                theaterRef={theaterRef}
                onNext={() => {
                  const nextEvent = matchingEvents.at(selectedIndex + 1)
                  if (nextEvent !== undefined) dispatch({ kind: 'event-selected', eventId: nextEvent.id })
                }}
                onPrevious={() => {
                  const previousEvent = matchingEvents.at(selectedIndex - 1)
                  if (previousEvent !== undefined) dispatch({ kind: 'event-selected', eventId: previousEvent.id })
                }}
              />
            )}
          </main>
          {directoryInput(fallbackInputRef, changeDirectory)}
        </div>
      )
    }
  }
}

function compatibilityStatus() {
  return (
    <p aria-label="ブラウザ互換性" className="compatibility-status" role="status">
      このブラウザでは互換モードを使用します。フォルダ選択画面からTeslaCamフォルダを選んでください。
    </p>
  )
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
