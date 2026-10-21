import { useEffect, useRef, type RefObject } from 'react'

/**
 * True when keyboard focus has nowhere meaningful to be: on `<body>`, or still on an element that a
 * screen change just removed or hid (browsers move such focus to `<body>` only on the next frame).
 */
export function focusIsLost(doc: Document = document): boolean {
  const active = doc.activeElement
  if (active === null || active === doc.body) return true
  return !active.isConnected || active.closest('[hidden], [inert]') !== null
}

/**
 * Moves focus to `target` when `active` turns true (and on mount when `onMount` is set), but only when
 * the screen change left focus nowhere, so it never takes focus away from where the user put it.
 */
export function useFocusHandoff(
  target: RefObject<HTMLElement | null>,
  active: boolean,
  { onMount = false }: { readonly onMount?: boolean } = {},
): void {
  const previous = useRef<boolean | null>(null)

  useEffect(() => {
    const was = previous.current
    previous.current = active
    const turnedOn = was === null ? onMount && active : !was && active
    if (turnedOn && focusIsLost()) target.current?.focus()
  }, [active, onMount, target])
}
