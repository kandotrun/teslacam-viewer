import { useCallback, useMemo, useState, type Dispatch } from 'react'

import type { AppAction, ReadyState } from '../app-state'
import type { TimelineDay } from '../day-timeline'
import {
  buildLibraryView,
  pickDateActions,
  selectDayActions,
  selectEventActions,
} from '../presentation/library-view'
import type { LayoutBand, ViewMode } from '../presentation/stage-layout'
import { LibraryBrowser } from './library/LibraryBrowser'
import { LibraryPanel } from './library/LibraryPanel'
import { EmptyStage } from './viewer/EmptyStage'
import { ViewerSession } from './viewer/ViewerSession'

type ReadyWorkspaceProps = {
  readonly state: ReadyState
  readonly dispatch: Dispatch<AppAction>
  readonly band: LayoutBand
  readonly onRescan: () => void
}

/**
 * The loaded library on the garage floor. Holds workspace-only UI state (view mode, search, hidden
 * library, phone pane); everything about the selection lives in the app state.
 */
export function ReadyWorkspace({ state, dispatch, band, onRescan }: ReadyWorkspaceProps) {
  const [viewMode, setViewMode] = useState<ViewMode>('spatial')
  const [searchOpen, setSearchOpen] = useState(false)
  const [libraryHidden, setLibraryHidden] = useState(false)
  const [phonePane, setPhonePane] = useState<'library' | 'viewer'>('library')
  const [revealEventId, setRevealEventId] = useState<string | null>(null)
  const view = useMemo(() => buildLibraryView(state), [state])
  const selectedEvent = view.selectedEvent

  if (revealEventId === null && selectedEvent !== undefined) setRevealEventId(selectedEvent.id)

  const run = useCallback((actions: readonly AppAction[]): void => {
    for (const action of actions) dispatch(action)
  }, [dispatch])

  const selectEvent = useCallback((eventId: string): void => {
    run(selectEventActions(state, eventId))
    setPhonePane('viewer')
  }, [run, state])

  const navigate = useCallback((eventId: string | null): (() => void) | null => (
    eventId === null ? null : () => run(selectEventActions(state, eventId))
  ), [run, state])

  const showSession = selectedEvent !== undefined && (band !== 'phone' || phonePane === 'viewer')
  const libraryHiddenNow = band === 'phone' ? showSession : band === 'tablet' && libraryHidden && selectedEvent !== undefined

  return (
    <div
      className="floor"
      data-band={band}
      data-library-hidden={libraryHiddenNow}
      data-phase="ready"
      data-session={showSession}
    >
      <LibraryPanel
        folderName={state.folderName}
        hidden={libraryHiddenNow}
        searchOpen={searchOpen}
        onRescan={onRescan}
        onToggleSearch={() => setSearchOpen((open) => !open)}
      >
        <LibraryBrowser
          filters={state.filters}
          searchOpen={searchOpen}
          selectedEventId={state.selectedEventId}
          view={view}
          warnings={state.library.warnings}
          onCategory={(category) => dispatch({ kind: 'category-filtered', category })}
          onCloseSearch={() => setSearchOpen(false)}
          onPickDate={(dateKey) => run(pickDateActions(view, dateKey))}
          onQuery={(query) => dispatch({ kind: 'query-filtered', query })}
          onSelectDay={(day: TimelineDay) => run(selectDayActions(day))}
          onSelectEvent={selectEvent}
        />
      </LibraryPanel>
      {showSession ? (
        <ViewerSession
          band={band}
          event={selectedEvent}
          libraryToggle={band === 'tablet' ? { hidden: libraryHidden, onToggle: () => setLibraryHidden((hidden) => !hidden) } : undefined}
          reveal={selectedEvent.id === revealEventId}
          viewMode={viewMode}
          onBack={band === 'phone' ? () => setPhonePane('library') : undefined}
          onNewer={navigate(view.newerEventId)}
          onOlder={navigate(view.olderEventId)}
          onViewMode={setViewMode}
        />
      ) : band !== 'phone' && (
        <>
          <header className="topbar">
            <section aria-labelledby="placeholder-title" className="pill placeholder-pill">
              <h1 id="placeholder-title">イベントを選択してください</h1>
              <p>一覧から選ぶと、記録された映像をここに表示します。</p>
            </section>
          </header>
          <EmptyStage />
        </>
      )}
    </div>
  )
}
