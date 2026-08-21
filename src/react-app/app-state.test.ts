import type { TeslaCamLibrary } from '../teslacam/types'

import { appReducer, initialAppState } from './app-state'

const library: TeslaCamLibrary = {
  events: [],
  eventsByCategory: { recent: [], saved: [], sentry: [] },
  warnings: [],
}

describe('appReducer', () => {
  it('moves from idle through scanning to a ready library', () => {
    const scanning = appReducer(initialAppState, { kind: 'scan-started' })

    expect(scanning).toMatchObject({
      kind: 'scanning',
      progress: { processed: 0, total: 0 },
    })

    const progressed = appReducer(scanning, {
      kind: 'scan-progressed',
      progress: { processed: 250, total: 500 },
    })
    const ready = appReducer(progressed, {
      kind: 'library-ready',
      folderName: 'TeslaCam',
      library,
    })

    expect(ready).toMatchObject({
      kind: 'ready',
      folderName: 'TeslaCam',
      library,
    })
  })

  it('returns cancellation to an idle state', () => {
    const scanning = appReducer(initialAppState, { kind: 'scan-started' })

    expect(appReducer(scanning, { kind: 'scan-cancelled' })).toEqual(
      initialAppState,
    )
  })

  it('keeps denied and invalid folders out of the ready state', () => {
    const scanning = appReducer(initialAppState, { kind: 'scan-started' })
    const denied = appReducer(scanning, { kind: 'scan-denied' })
    const invalid = appReducer(scanning, { kind: 'scan-invalid' })

    expect(denied).toMatchObject({ kind: 'error', reason: 'denied' })
    expect(invalid).toMatchObject({ kind: 'error', reason: 'invalid' })
  })

  it('retains partial scan warnings in the ready state', () => {
    const warnedLibrary: TeslaCamLibrary = {
      ...library,
      warnings: [{ kind: 'encrypted-file', count: 2 }],
    }
    const ready = appReducer(initialAppState, {
      kind: 'library-ready',
      folderName: 'TeslaCam',
      library: warnedLibrary,
    })

    expect(ready).toMatchObject({
      kind: 'ready',
      library: { warnings: [{ kind: 'encrypted-file', count: 2 }] },
    })
  })

  it('updates category, date, text filters, selected event, and rescan state', () => {
    const ready = appReducer(initialAppState, {
      kind: 'library-ready',
      folderName: 'TeslaCam',
      library,
    })
    const filtered = appReducer(
      appReducer(
        appReducer(ready, { kind: 'category-filtered', category: 'sentry' }),
        { kind: 'date-filtered', dateKey: '2026-08-20' },
      ),
      { kind: 'query-filtered', query: '20:45' },
    )
    const selected = appReducer(filtered, {
      kind: 'event-selected',
      eventId: 'sentry-2026-08-20_20-45-00-1',
    })

    expect(selected).toMatchObject({
      kind: 'ready',
      filters: { category: 'sentry', dateKey: '2026-08-20', query: '20:45' },
      selectedEventId: 'sentry-2026-08-20_20-45-00-1',
    })
    expect(appReducer(selected, { kind: 'scan-started' })).toMatchObject({
      kind: 'scanning',
      progress: { processed: 0, total: 0 },
    })
  })
})
