import { useSyncExternalStore } from 'react'

export type TimelinePresentation = 'rail' | 'ledger' | 'mobile'

const mobileQuery = '(max-width: 639px)'
const ledgerQuery = '(max-width: 1024px)'

function presentationSnapshot(): TimelinePresentation {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return 'rail'
  if (window.matchMedia(mobileQuery).matches) return 'mobile'
  if (window.matchMedia(ledgerQuery).matches) return 'ledger'
  return 'rail'
}

function subscribePresentation(change: () => void): () => void {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return () => undefined
  const queries = [window.matchMedia(mobileQuery), window.matchMedia(ledgerQuery)]
  for (const query of queries) query.addEventListener('change', change)
  return () => {
    for (const query of queries) query.removeEventListener('change', change)
  }
}

export function useTimelinePresentation(): TimelinePresentation {
  return useSyncExternalStore(subscribePresentation, presentationSnapshot, () => 'rail')
}
