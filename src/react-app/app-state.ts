import type {
  ClipCategory,
  ScanProgress,
  TeslaCamEvent,
  TeslaCamLibrary,
} from '../teslacam/types'

export type EventFilters = {
  readonly category: ClipCategory | 'all'
  readonly dateKey: string | null
  readonly query: string
}

type AppErrorReason = 'denied' | 'invalid' | 'failed'

type IdleState = {
  readonly kind: 'idle'
  readonly filters: EventFilters
}

type ScanningState = {
  readonly kind: 'scanning'
  readonly progress: ScanProgress
}

type ReadyState = {
  readonly kind: 'ready'
  readonly folderName: string
  readonly library: TeslaCamLibrary
  readonly filters: EventFilters
  readonly selectedEventId: string | null
}

type ErrorState = {
  readonly kind: 'error'
  readonly reason: AppErrorReason
  readonly filters: EventFilters
}

export type AppState = IdleState | ScanningState | ReadyState | ErrorState

export type AppAction =
  | { readonly kind: 'scan-started' }
  | { readonly kind: 'scan-progressed'; readonly progress: ScanProgress }
  | { readonly kind: 'scan-cancelled' }
  | { readonly kind: 'scan-denied' }
  | { readonly kind: 'scan-invalid' }
  | { readonly kind: 'scan-failed' }
  | {
      readonly kind: 'library-ready'
      readonly folderName: string
      readonly library: TeslaCamLibrary
    }
  | { readonly kind: 'category-filtered'; readonly category: EventFilters['category'] }
  | { readonly kind: 'date-filtered'; readonly dateKey: string | null }
  | { readonly kind: 'query-filtered'; readonly query: string }
  | { readonly kind: 'event-selected'; readonly eventId: string }

const initialFilters: EventFilters = {
  category: 'all',
  dateKey: null,
  query: '',
}

export const initialAppState: AppState = {
  kind: 'idle',
  filters: initialFilters,
}

function assertNever(value: never): never {
  void value
  throw new Error('Unhandled app state')
}

function updateReady(
  state: AppState,
  update: (state: ReadyState) => ReadyState,
): AppState {
  switch (state.kind) {
    case 'ready':
      return update(state)
    case 'idle':
    case 'scanning':
    case 'error':
      return state
  }
}

export function eventTime(timestamp: string): string {
  return timestamp.replace('_', ' ').replaceAll('-', (match, offset) =>
    offset > 10 ? ':' : match,
  )
}

export function categoryLabel(category: ClipCategory): string {
  switch (category) {
    case 'recent':
      return '最近の走行'
    case 'saved':
      return '保存済み'
    case 'sentry':
      return 'セントリー'
  }
}

export function eventMatchesFilters(event: TeslaCamEvent, filters: EventFilters): boolean {
  if (filters.category !== 'all' && event.category !== filters.category) return false
  if (filters.dateKey !== null && !event.timestamp.startsWith(filters.dateKey)) return false
  const query = filters.query.trim().toLowerCase()
  return (
    query === '' ||
    `${eventTime(event.timestamp)} ${event.category} ${categoryLabel(event.category)}`
      .toLowerCase()
      .includes(query)
  )
}

export function orderedMatchingEvents(
  events: readonly TeslaCamEvent[],
  filters: EventFilters,
): readonly TeslaCamEvent[] {
  return events
    .filter((event) => eventMatchesFilters(event, filters))
    .sort((left, right) => right.timestamp.localeCompare(left.timestamp))
}

function withFilters(ready: ReadyState, filters: EventFilters): ReadyState {
  const selectedEvent = ready.library.events.find((event) => event.id === ready.selectedEventId)
  return {
    ...ready,
    filters,
    selectedEventId:
      selectedEvent !== undefined && eventMatchesFilters(selectedEvent, filters)
        ? ready.selectedEventId
        : null,
  }
}

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.kind) {
    case 'scan-started':
      return { kind: 'scanning', progress: { processed: 0, total: 0 } }
    case 'scan-progressed':
      switch (state.kind) {
        case 'scanning':
          return { ...state, progress: action.progress }
        case 'idle':
        case 'ready':
        case 'error':
          return state
        default:
          return assertNever(state)
      }
    case 'scan-cancelled':
      return initialAppState
    case 'scan-denied':
      return { kind: 'error', reason: 'denied', filters: initialFilters }
    case 'scan-invalid':
      return { kind: 'error', reason: 'invalid', filters: initialFilters }
    case 'scan-failed':
      return { kind: 'error', reason: 'failed', filters: initialFilters }
    case 'library-ready':
      if (action.library.events.length === 0) {
        return { kind: 'error', reason: 'invalid', filters: initialFilters }
      }
      return {
        kind: 'ready',
        folderName: action.folderName,
        library: action.library,
        filters: initialFilters,
        selectedEventId: null,
      }
    case 'category-filtered':
      return updateReady(state, (ready) =>
        withFilters(ready, { ...ready.filters, category: action.category }),
      )
    case 'date-filtered':
      return updateReady(state, (ready) =>
        withFilters(ready, { ...ready.filters, dateKey: action.dateKey }),
      )
    case 'query-filtered':
      return updateReady(state, (ready) =>
        withFilters(ready, { ...ready.filters, query: action.query }),
      )
    case 'event-selected':
      return updateReady(state, (ready) => ({
        ...ready,
        selectedEventId: action.eventId,
      }))
    default:
      return assertNever(action)
  }
}
