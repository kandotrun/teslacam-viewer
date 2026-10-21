import { focusIsLost } from './use-focus-handoff'

afterEach(() => {
  document.body.replaceChildren()
})

describe('focusIsLost', () => {
  it('is true on <body> and on an element inside a hidden region, false on a visible control', () => {
    const visible = document.createElement('button')
    const panel = document.createElement('aside')
    const hiddenRow = document.createElement('button')
    panel.append(hiddenRow)
    document.body.append(visible, panel)

    expect(focusIsLost()).toBe(true)
    visible.focus()
    expect(focusIsLost()).toBe(false)
    hiddenRow.focus()
    panel.hidden = true
    expect(focusIsLost()).toBe(true)
  })
})
