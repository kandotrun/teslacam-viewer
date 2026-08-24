import { useReducer, useRef } from 'react'

import type { LocalFileRef, TeslaCamEvent, TeslaCamLibrary } from '../teslacam/types'
import { appReducer, type ReadyState } from './app-state'
import { ReadyWorkspace } from './components/ReadyWorkspace'

function fileRef(name: string): LocalFileRef {
  return {
    name,
    relativePath: `SentryClips/${name}`,
    size: 4,
    type: 'video/mp4',
    getFile: async () => new File(['clip'], name, { type: 'video/mp4' }),
  }
}

function sentryEvent(id: string, segments: readonly [string, string], detectionTimestamp: string): TeslaCamEvent {
  return {
    id,
    category: 'sentry',
    timestamp: segments[0],
    metadata: { timestamp: detectionTimestamp },
    segments: segments.map((timestamp) => ({ timestamp, cameras: { front: fileRef(`${id}-${timestamp}-front.mp4`) } })),
  }
}

const preRollEvent = sentryEvent('sentry-preroll', ['2026-08-21_12-00-00', '2026-08-21_12-01-00'], '2026-08-21T12:01:20')
const startEvent = sentryEvent('sentry-start', ['2026-08-21_13-00-00', '2026-08-21_13-01-00'], '2026-08-21T13:00:10')
const selectionLibrary: TeslaCamLibrary = {
  events: [startEvent, preRollEvent],
  eventsByCategory: { recent: [], saved: [], sentry: [startEvent, preRollEvent] },
  warnings: [],
}
const selectionState: ReadyState = {
  kind: 'ready',
  folderName: 'TeslaCam',
  library: selectionLibrary,
  filters: { category: 'all', dateKey: null, query: '' },
  selectedEventId: preRollEvent.id,
}

export function WorkspaceSelectionHarness() {
  const [state, dispatch] = useReducer(appReducer, selectionState)
  const theaterRef = useRef<HTMLElement>(null)
  if (state.kind !== 'ready') throw new Error('workspace state is not ready')
  return (
    <ReadyWorkspace
      dispatch={dispatch}
      state={state}
      theaterRef={theaterRef}
      onRescan={() => undefined}
      onSelectEvent={(eventId) => dispatch({ kind: 'event-selected', eventId })}
    />
  )
}
