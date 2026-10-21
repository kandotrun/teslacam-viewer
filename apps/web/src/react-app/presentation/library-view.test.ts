import type { ClipCategory, TeslaCamEvent, TeslaCamLibrary } from '../../teslacam/types'
import type { ReadyState } from '../app-state'
import { buildLibraryView, pickDateActions, selectDayActions, selectEventActions } from './library-view'

function event(id: string, category: ClipCategory, timestamp: string): TeslaCamEvent {
  return { id, category, timestamp, segments: [{ timestamp, cameras: {} }] }
}

const recent = event('recent-0820', 'recent', '2026-08-20_20-45-00')
const saved = event('saved-0820', 'saved', '2026-08-20_20-40-00')
const sentry = event('sentry-0820', 'sentry', '2026-08-20_20-35-00')
const previous = event('saved-0819', 'saved', '2026-08-19_19-30-00')
const library: TeslaCamLibrary = {
  events: [recent, saved, sentry, previous],
  eventsByCategory: { recent: [recent], saved: [saved, previous], sentry: [sentry] },
  warnings: [],
}

function ready(overrides: Partial<Pick<ReadyState, 'filters' | 'selectedEventId'>> = {}): ReadyState {
  return {
    kind: 'ready',
    folderName: 'TeslaCam',
    library,
    filters: { category: 'all', dateKey: null, query: '' },
    selectedEventId: null,
    ...overrides,
  }
}

describe('buildLibraryView', () => {
  it('shows the newest recorded day when nothing is selected', () => {
    const view = buildLibraryView(ready())

    expect(view.counts).toEqual({ all: 4, recent: 1, saved: 2, sentry: 1 })
    expect(view.dayKey).toBe('2026-08-20')
    expect(view.dayEvents.map((item) => item.id)).toEqual(['recent-0820', 'saved-0820', 'sentry-0820'])
    expect(view.markers.map((marker) => marker.eventId)).toEqual(['sentry-0820', 'saved-0820', 'recent-0820'])
    expect(view.previousDay?.dateKey).toBe('2026-08-19')
    expect(view.nextDay).toBeUndefined()
  })

  it('follows the selected event and offers older and newer events across days', () => {
    const view = buildLibraryView(ready({ selectedEventId: 'sentry-0820' }))

    expect(view.olderEventId).toBe('saved-0819')
    expect(view.newerEventId).toBe('saved-0820')
    expect(buildLibraryView(ready({ selectedEventId: 'recent-0820' })).newerEventId).toBeNull()
  })

  it('narrows rows and rail markers by category without hiding other days', () => {
    const view = buildLibraryView(ready({ filters: { category: 'saved', dateKey: null, query: '' } }))

    expect(view.dayEvents.map((item) => item.id)).toEqual(['saved-0820'])
    expect(view.previousDay?.eventIds).toEqual(['saved-0819'])
  })

  it('shows an empty day for a picked date without recordings', () => {
    const view = buildLibraryView(ready({ filters: { category: 'all', dateKey: '2026-08-10', query: '' } }))

    expect(view.dayKey).toBe('2026-08-10')
    expect(view.dayEvents).toEqual([])
    expect(view.markers).toEqual([])
  })
})

describe('selection actions', () => {
  it('moves an active day filter along with an event on another day', () => {
    const state = ready({ filters: { category: 'all', dateKey: '2026-08-20', query: '' }, selectedEventId: 'sentry-0820' })

    expect(selectEventActions(state, 'saved-0819')).toEqual([
      { kind: 'date-filtered', dateKey: '2026-08-19' },
      { kind: 'event-selected', eventId: 'saved-0819' },
    ])
    expect(selectEventActions(ready(), 'saved-0819')).toEqual([{ kind: 'event-selected', eventId: 'saved-0819' }])
  })

  it('selects the newest event when moving to a recorded day', () => {
    expect(selectDayActions({ dateKey: '2026-08-20', eventIds: ['sentry-0820', 'saved-0820', 'recent-0820'] })).toEqual([
      { kind: 'date-filtered', dateKey: '2026-08-20' },
      { kind: 'event-selected', eventId: 'recent-0820' },
    ])
  })

  it('filters to a picked date and plays its newest event when it has recordings', () => {
    const view = buildLibraryView(ready())

    expect(pickDateActions(view, '2026-08-19')).toEqual([
      { kind: 'date-filtered', dateKey: '2026-08-19' },
      { kind: 'event-selected', eventId: 'saved-0819' },
    ])
    expect(pickDateActions(view, '2026-08-10')).toEqual([{ kind: 'date-filtered', dateKey: '2026-08-10' }])
  })
})
