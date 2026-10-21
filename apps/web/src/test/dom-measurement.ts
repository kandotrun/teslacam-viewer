import { vi } from 'vitest'

/**
 * jsdom has no layout: give every element the same measured size and a ResizeObserver that reports
 * once on observe, so components that measure themselves (the day rail, the stage) can be tested.
 */
export function mockElementSize(width: number, height = 40): void {
  vi.spyOn(Element.prototype, 'getBoundingClientRect').mockReturnValue(DOMRect.fromRect({ x: 0, y: 0, width, height }))
  class ImmediateResizeObserver {
    constructor(private readonly callback: ResizeObserverCallback) {}

    observe(): void {
      this.callback([], this)
    }

    unobserve(): void {}

    disconnect(): void {}
  }
  vi.stubGlobal('ResizeObserver', ImmediateResizeObserver)
}
