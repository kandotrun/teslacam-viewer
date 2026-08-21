import type { ClipCategory, ScanProgress, TeslaCamLibrary } from '../teslacam/types'

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
      return {
        kind: 'ready',
        folderName: action.folderName,
        library: action.library,
        filters: initialFilters,
        selectedEventId: null,
      }
    case 'category-filtered':
      return updateReady(state, (ready) => ({
        ...ready,
        filters: { ...ready.filters, category: action.category },
      }))
    case 'date-filtered':
      return updateReady(state, (ready) => ({
        ...ready,
        filters: { ...ready.filters, dateKey: action.dateKey },
      }))
    case 'query-filtered':
      return updateReady(state, (ready) => ({
        ...ready,
        filters: { ...ready.filters, query: action.query },
      }))
    case 'event-selected':
      return updateReady(state, (ready) => ({
        ...ready,
        selectedEventId: action.eventId,
      }))
    default:
      return assertNever(action)
  }
}
