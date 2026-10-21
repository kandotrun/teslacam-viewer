import { useSyncExternalStore } from 'react'

import { designTokens } from '../design/contract'
import type { LayoutBand } from '../presentation/stage-layout'

const phoneQuery = `(max-width: ${designTokens.breakpoint['phone-max']}px)`
const tabletQuery = `(max-width: ${designTokens.breakpoint['tablet-max']}px)`

function hasMatchMedia(): boolean {
  return typeof window !== 'undefined' && typeof window.matchMedia === 'function'
}

function snapshot(): LayoutBand {
  if (!hasMatchMedia()) return 'desktop'
  if (window.matchMedia(phoneQuery).matches) return 'phone'
  if (window.matchMedia(tabletQuery).matches) return 'tablet'
  return 'desktop'
}

function subscribe(change: () => void): () => void {
  if (!hasMatchMedia()) return () => undefined
  const queries = [window.matchMedia(phoneQuery), window.matchMedia(tabletQuery)]
  for (const query of queries) query.addEventListener('change', change)
  return () => {
    for (const query of queries) query.removeEventListener('change', change)
  }
}

/** Desktop (≥ 1101 px), tablet (741–1100 px) or phone (≤ 740 px), as in DESIGN.md §5. */
export function useLayoutBand(): LayoutBand {
  return useSyncExternalStore(subscribe, snapshot, () => 'desktop')
}
