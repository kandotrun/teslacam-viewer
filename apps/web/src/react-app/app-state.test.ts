import type { LocalFileRef, TeslaCamEvent, TeslaCamLibrary } from '../teslacam/types'

import { appReducer, initialAppState } from './app-state'
import { orderedMatchingEvents } from './event-filters'

const file: LocalFileRef = {
  name: 'front.mp4',
  relativePath: 'SentryClips/2026-08-20_20-45-00-front.mp4',
  size: 12,
  type: 'video/mp4',
  getFile: async () => new File(['clip'], 'front.mp4'),
}

const event: TeslaCamEvent = {
  id: 'sentry-2026-08-20_20-45-00',
  category: 'sentry',
  timestamp: '2026-08-20_20-45-00',
  segments: [
    {
      timestamp: '2026-08-20_20-45-00',
      cameras: { front: file },
    },
  ],
}

const library: TeslaCamLibrary = {
  events: [event],
  eventsByCategory: { recent: [], saved: [], sentry: [event] },
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

  it('keeps the loaded library when a re-pick is cancelled', () => {
    const ready = appReducer(appReducer(initialAppState, { kind: 'scan-started' }), {
      kind: 'library-ready',
      folderName: 'TeslaCam',
      library,
    })
    const selected = appReducer(ready, { kind: 'event-selected', eventId: event.id })

    expect(appReducer(selected, { kind: 'scan-cancelled' })).toBe(selected)
  })

  it('returns a cancelled retry from an error to the welcome state', () => {
    const failed = appReducer(initialAppState, { kind: 'scan-failed' })

    expect(appReducer(failed, { kind: 'scan-cancelled' })).toEqual(initialAppState)
  })

  it('keeps denied and invalid folders out of the ready state', () => {
    const scanning = appReducer(initialAppState, { kind: 'scan-started' })
    const denied = appReducer(scanning, { kind: 'scan-denied' })
    const invalid = appReducer(scanning, { kind: 'scan-invalid' })

    expect(denied).toMatchObject({ kind: 'error', reason: 'denied' })
    expect(invalid).toMatchObject({ kind: 'error', reason: 'invalid' })
  })

  it('keeps an empty library in the invalid error state', () => {
    const emptyLibrary: TeslaCamLibrary = {
      events: [],
      eventsByCategory: { recent: [], saved: [], sentry: [] },
      warnings: [],
    }

    expect(
      appReducer(initialAppState, {
        kind: 'library-ready',
        folderName: 'TeslaCam',
        library: emptyLibrary,
      }),
    ).toEqual({
      kind: 'error',
      reason: 'invalid',
      filters: { category: 'all', dateKey: null, query: '' },
    })
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

  it('updates category, date, and text filters and clears a selection they hide', () => {
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
      eventId: event.id,
    })

    expect(selected).toMatchObject({
      kind: 'ready',
      filters: { category: 'sentry', dateKey: '2026-08-20', query: '20:45' },
      selectedEventId: event.id,
    })
    const hidden = appReducer(selected, {
      kind: 'category-filtered',
      category: 'saved',
    })

    expect(hidden).toMatchObject({
      kind: 'ready',
      filters: { category: 'saved', dateKey: '2026-08-20', query: '20:45' },
      selectedEventId: null,
    })
    expect(appReducer(hidden, { kind: 'scan-started' })).toMatchObject({
      kind: 'scanning',
      progress: { processed: 0, total: 0 },
    })
  })
})

describe('orderedMatchingEvents', () => {
  it('returns only matching events in newest-first navigation order', () => {
    const newer: TeslaCamEvent = {
      ...event,
      id: 'saved-newer',
      category: 'saved',
      timestamp: '2026-08-21_09-00-00',
    }
    const older: TeslaCamEvent = {
      ...newer,
      id: 'saved-older',
      timestamp: '2026-08-20_09-00-00',
    }

    expect(orderedMatchingEvents([older, event, newer], {
      category: 'saved',
      dateKey: null,
      query: '',
    }).map((matchingEvent) => matchingEvent.id)).toEqual(['saved-newer', 'saved-older'])
  })
})
