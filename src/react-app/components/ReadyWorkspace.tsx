import type { Dispatch, RefObject } from 'react'

import { orderedMatchingEvents } from '../event-filters'
import type { AppAction, ReadyState } from '../app-state'
import { AppHeader } from './AppHeader'
import { EventList } from './EventList'
import { LibrarySidebar } from './LibrarySidebar'
import { Theater } from './Theater'

type ReadyWorkspaceProps = {
  readonly state: ReadyState
  readonly dispatch: Dispatch<AppAction>
  readonly theaterRef: RefObject<HTMLElement | null>
  readonly onRescan: () => void
  readonly onSelectEvent: (eventId: string) => void
}

export function ReadyWorkspace({
  state,
  dispatch,
  theaterRef,
  onRescan,
  onSelectEvent,
}: ReadyWorkspaceProps) {
  const matchingEvents = orderedMatchingEvents(state.library.events, state.filters)
  const selectedEvent = state.library.events.find((event) => event.id === state.selectedEventId)
  const selectedIndex = matchingEvents.findIndex((event) => event.id === state.selectedEventId)
  const counts = {
    recent: state.library.eventsByCategory.recent.length,
    saved: state.library.eventsByCategory.saved.length,
    sentry: state.library.eventsByCategory.sentry.length,
  }

  return (
    <div className="library-shell">
      <AppHeader folderName={state.folderName} onRescan={onRescan} />
      <main className="library-workspace">
        <LibrarySidebar
          counts={counts}
          eventCount={state.library.events.length}
          filters={state.filters}
          folderName={state.folderName}
          warnings={state.library.warnings}
          onCategory={(category) => dispatch({ kind: 'category-filtered', category })}
          onDate={(dateKey) => dispatch({ kind: 'date-filtered', dateKey })}
          onQuery={(query) => dispatch({ kind: 'query-filtered', query })}
        />
        <EventList
          events={matchingEvents}
          selectedEventId={state.selectedEventId}
          onSelectEvent={onSelectEvent}
        />
        {selectedEvent === undefined ? (
          <section className="selection-placeholder" aria-labelledby="selection-title">
            <h1 id="selection-title">イベントを選択してください</h1>
            <p className="selection-description">一覧から選ぶと、記録された映像をここに表示します。</p>
          </section>
        ) : (
          <Theater
            event={selectedEvent}
            hasNext={selectedIndex >= 0 && selectedIndex < matchingEvents.length - 1}
            hasPrevious={selectedIndex > 0}
            selectedEventId={state.selectedEventId}
            sourceName={state.folderName}
            theaterRef={theaterRef}
            timelineDateKey={state.filters.dateKey}
            timelineEvents={matchingEvents}
            onNext={() => {
              const nextEvent = matchingEvents.at(selectedIndex + 1)
              if (nextEvent !== undefined) {
                dispatch({ kind: 'event-selected', eventId: nextEvent.id })
              }
            }}
            onPrevious={() => {
              const previousEvent = matchingEvents.at(selectedIndex - 1)
              if (previousEvent !== undefined) {
                dispatch({ kind: 'event-selected', eventId: previousEvent.id })
              }
            }}
            onSelectEvent={onSelectEvent}
          />
        )}
      </main>
    </div>
  )
}
