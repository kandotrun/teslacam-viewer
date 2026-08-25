import { useState, type Dispatch, type RefObject } from 'react'

import { orderedMatchingEvents } from '../event-filters'
import type { AppAction, ReadyState } from '../app-state'
import { AppHeader } from './AppHeader'
import { EventNavigator } from './EventNavigator'
import { Theater } from './Theater'
import type { TheaterViewMode } from './TheaterViewSwitch'

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
  const [viewMode, setViewMode] = useState<TheaterViewMode>('spatial')
  const matchingEvents = orderedMatchingEvents(state.library.events, state.filters)
  const timelineEvents = orderedMatchingEvents(state.library.events, {
    ...state.filters,
    dateKey: null,
  })
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
        <EventNavigator
          events={matchingEvents}
          selectedEventId={state.selectedEventId}
          counts={counts}
          eventCount={state.library.events.length}
          filters={state.filters}
          folderName={state.folderName}
          warnings={state.library.warnings}
          onCategory={(category) => dispatch({ kind: 'category-filtered', category })}
          onDate={(dateKey) => dispatch({ kind: 'date-filtered', dateKey })}
          onQuery={(query) => dispatch({ kind: 'query-filtered', query })}
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
            timelineEvents={timelineEvents}
            viewMode={viewMode}
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
            onSelectTimelineDate={({ dateKey, eventId }) => {
              dispatch({ kind: 'date-filtered', dateKey })
              onSelectEvent(eventId)
            }}
            onViewMode={setViewMode}
          />
        )}
      </main>
    </div>
  )
}
