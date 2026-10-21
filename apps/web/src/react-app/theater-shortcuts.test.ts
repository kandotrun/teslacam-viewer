import { shortcutFocus, theaterShortcut, type ShortcutInput } from './theater-shortcuts'

const base: ShortcutInput = {
  key: ' ',
  modifiers: { meta: false, ctrl: false, alt: false },
  focus: 'none',
  expandedCamera: null,
  playing: false,
  dialogOpen: false,
}

describe('theaterShortcut', () => {
  it.each([
    [' ', false, { action: 'play' }],
    [' ', true, { action: 'pause' }],
    ['ArrowLeft', false, { action: 'skip', seconds: -10 }],
    ['ArrowRight', false, { action: 'skip', seconds: 10 }],
    ['1', false, { action: 'toggleCamera', camera: 'front' }],
    ['2', false, { action: 'toggleCamera', camera: 'back' }],
    ['6', false, { action: 'toggleCamera', camera: 'right_repeater' }],
    ['[', false, { action: 'olderEvent' }],
    [']', false, { action: 'newerEvent' }],
    ['7', false, { action: 'none' }],
    ['Escape', false, { action: 'none' }],
  ])('maps %s (playing %s)', (key, playing, expected) => {
    expect(theaterShortcut({ ...base, key, playing })).toEqual(expected)
  })

  it('collapses an expanded camera with Escape even from a focused control', () => {
    expect(theaterShortcut({ ...base, key: 'Escape', focus: 'button', expandedCamera: 'back' })).toEqual({ action: 'collapse' })
  })

  it('leaves keys to a focused control, a modifier chord or an open dialog', () => {
    expect(theaterShortcut({ ...base, key: 'ArrowRight', focus: 'button' })).toEqual({ action: 'none' })
    expect(theaterShortcut({ ...base, key: '6', focus: 'text' })).toEqual({ action: 'none' })
    expect(theaterShortcut({ ...base, key: '1', modifiers: { meta: true, ctrl: false, alt: false } })).toEqual({ action: 'none' })
    expect(theaterShortcut({ ...base, key: ' ', dialogOpen: true })).toEqual({ action: 'none' })
  })
})

describe('shortcutFocus', () => {
  it('classifies the focused element', () => {
    const radio = document.createElement('div')
    radio.setAttribute('role', 'radio')
    expect(shortcutFocus(document.createElement('button'))).toBe('button')
    expect(shortcutFocus(document.createElement('input'))).toBe('text')
    expect(shortcutFocus(document.createElement('select'))).toBe('select')
    expect(shortcutFocus(radio)).toBe('button')
    expect(shortcutFocus(document.body)).toBe('none')
    expect(shortcutFocus(null)).toBe('none')
  })
})
