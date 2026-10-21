import { scrollOffsetFor, visibleRowWindow } from './use-virtual-rows'

describe('visibleRowWindow', () => {
  it('mounts only the rows near the viewport', () => {
    expect(visibleRowWindow(1000, 90, 9000, 450, 2)).toEqual({ start: 98, end: 107, totalHeight: 90_000 })
  })

  it('clamps to the list bounds', () => {
    expect(visibleRowWindow(5, 90, 0, 900, 4)).toEqual({ start: 0, end: 5, totalHeight: 450 })
  })

  it('draws a first page before the viewport is measured', () => {
    expect(visibleRowWindow(100, 90, 0, 0, 4)).toEqual({ start: 0, end: 12, totalHeight: 9000 })
  })
})

describe('scrollOffsetFor', () => {
  it('scrolls a row above or below the viewport into view and leaves visible rows alone', () => {
    expect(scrollOffsetFor(2, 90, 400, 300)).toBe(180)
    expect(scrollOffsetFor(10, 90, 0, 300)).toBe(690)
    expect(scrollOffsetFor(1, 90, 0, 300)).toBeNull()
  })
})
