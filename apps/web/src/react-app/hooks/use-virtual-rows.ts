import { useCallback, useEffect, useState, type RefObject } from 'react'

export type VirtualWindow = {
  readonly start: number
  readonly end: number
  readonly totalHeight: number
}

/** Rows drawn when the viewport cannot be measured (for example before layout). */
const unmeasuredRowCount = 12

/** The visible slice of a list of fixed-height rows, plus `overscan` rows on each side. Pure. */
export function visibleRowWindow(
  count: number,
  rowStride: number,
  scrollTop: number,
  viewportHeight: number,
  overscan: number,
): VirtualWindow {
  const totalHeight = count * rowStride
  if (viewportHeight <= 0) return { start: 0, end: Math.min(count, unmeasuredRowCount), totalHeight }
  const first = Math.floor(scrollTop / rowStride)
  const last = Math.ceil((scrollTop + viewportHeight) / rowStride)
  return {
    start: Math.max(0, first - overscan),
    end: Math.min(count, last + overscan),
    totalHeight,
  }
}

/** Scroll offset that brings row `index` fully into view, or null when it already is. */
export function scrollOffsetFor(index: number, rowStride: number, scrollTop: number, viewportHeight: number): number | null {
  const top = index * rowStride
  const bottom = top + rowStride
  if (top < scrollTop) return top
  if (viewportHeight > 0 && bottom > scrollTop + viewportHeight) return bottom - viewportHeight
  return null
}

/** Windowed rendering for the event list: only rows near the viewport are mounted. */
export function useVirtualRows(
  scrollerRef: RefObject<HTMLElement | null>,
  count: number,
  rowStride: number,
  overscan = 4,
): VirtualWindow & { readonly scrollToIndex: (index: number) => void } {
  const [viewport, setViewport] = useState({ scrollTop: 0, height: 0 })

  useEffect(() => {
    const scroller = scrollerRef.current
    if (scroller === null) return undefined
    const update = (): void => {
      setViewport((current) => (
        current.scrollTop === scroller.scrollTop && current.height === scroller.clientHeight
          ? current
          : { scrollTop: scroller.scrollTop, height: scroller.clientHeight }
      ))
    }
    scroller.addEventListener('scroll', update, { passive: true })
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(update)
    observer?.observe(scroller)
    update()
    return () => {
      scroller.removeEventListener('scroll', update)
      observer?.disconnect()
    }
  }, [scrollerRef])

  const scrollToIndex = useCallback((index: number): void => {
    const scroller = scrollerRef.current
    if (scroller === null || index < 0) return
    const offset = scrollOffsetFor(index, rowStride, scroller.scrollTop, scroller.clientHeight)
    if (offset !== null) scroller.scrollTop = offset
  }, [rowStride, scrollerRef])

  return { ...visibleRowWindow(count, rowStride, viewport.scrollTop, viewport.height, overscan), scrollToIndex }
}
