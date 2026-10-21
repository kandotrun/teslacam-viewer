/**
 * Keyboard shortcuts of the viewer (DESIGN.md §9, parity spec §2.10.10) as a pure decision, so the
 * window listener only translates DOM events and applies the result.
 */
import { cameraIds, type CameraId } from '../teslacam/types'

export type ShortcutFocus = 'none' | 'button' | 'text' | 'select' | 'editable'

export type ShortcutInput = {
  readonly key: string
  readonly modifiers: { readonly meta: boolean; readonly ctrl: boolean; readonly alt: boolean }
  readonly focus: ShortcutFocus
  readonly expandedCamera: CameraId | null
  readonly playing: boolean
  readonly dialogOpen: boolean
}

export type ShortcutAction =
  | { readonly action: 'none' }
  | { readonly action: 'collapse' }
  | { readonly action: 'play' }
  | { readonly action: 'pause' }
  | { readonly action: 'skip'; readonly seconds: number }
  | { readonly action: 'toggleCamera'; readonly camera: CameraId }
  | { readonly action: 'olderEvent' }
  | { readonly action: 'newerEvent' }

const none: ShortcutAction = { action: 'none' }

export function theaterShortcut(input: ShortcutInput): ShortcutAction {
  if (input.dialogOpen) return none
  if (input.modifiers.meta || input.modifiers.ctrl || input.modifiers.alt) return none
  // Escape returns to the layout even when a camera window or another control has focus.
  if (input.key === 'Escape') return input.expandedCamera === null ? none : { action: 'collapse' }
  if (input.focus !== 'none') return none
  switch (input.key) {
    case ' ':
      return { action: input.playing ? 'pause' : 'play' }
    case 'ArrowLeft':
      return { action: 'skip', seconds: -10 }
    case 'ArrowRight':
      return { action: 'skip', seconds: 10 }
    case '[':
      return { action: 'olderEvent' }
    case ']':
      return { action: 'newerEvent' }
    default: {
      const digit = /^[1-6]$/u.test(input.key) ? Number(input.key) : 0
      const camera = cameraIds[digit - 1]
      return camera === undefined ? none : { action: 'toggleCamera', camera }
    }
  }
}

/** Where keyboard focus is, as far as shortcuts care. */
export function shortcutFocus(target: EventTarget | null): ShortcutFocus {
  if (!(target instanceof HTMLElement)) return 'none'
  if (target.isContentEditable) return 'editable'
  switch (target.tagName) {
    case 'INPUT':
    case 'TEXTAREA':
      return 'text'
    case 'SELECT':
      return 'select'
    case 'BUTTON':
      return 'button'
    default:
      return target.getAttribute('role') === 'radio' || target.getAttribute('role') === 'menuitem' ? 'button' : 'none'
  }
}
