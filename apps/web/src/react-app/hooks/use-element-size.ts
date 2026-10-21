import { useEffect, useState, type RefObject } from 'react'

export type ElementSize = { readonly width: number; readonly height: number }

const unmeasured: ElementSize = { width: 0, height: 0 }

/** The element's border-box size, kept current with a ResizeObserver (0 × 0 where none exists). */
export function useElementSize(ref: RefObject<Element | null>): ElementSize {
  const [size, setSize] = useState<ElementSize>(unmeasured)
  useEffect(() => {
    const element = ref.current
    if (element === null || typeof ResizeObserver === 'undefined') return undefined
    const measure = (): void => {
      const bounds = element.getBoundingClientRect()
      setSize((current) => (current.width === bounds.width && current.height === bounds.height
        ? current
        : { width: bounds.width, height: bounds.height }))
    }
    const observer = new ResizeObserver(measure)
    observer.observe(element)
    measure()
    return () => observer.disconnect()
  }, [ref])
  return size
}
