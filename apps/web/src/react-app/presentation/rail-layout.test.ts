import { layoutRailItems, railPillsFit } from './rail-layout'

describe('layoutRailItems', () => {
  it('centres items that have room', () => {
    expect(layoutRailItems([{ center: 50, width: 44 }, { center: 200, width: 44 }], 300)).toEqual([28, 178])
  })

  it('keeps items inside the track', () => {
    expect(layoutRailItems([{ center: 5, width: 44 }, { center: 298, width: 96 }], 300)).toEqual([0, 204])
  })

  it('pushes a pill away from a neighbour instead of overlapping it', () => {
    const lefts = layoutRailItems([{ center: 100, width: 44 }, { center: 150, width: 96 }], 300)

    expect(lefts[0]).toBe(78)
    expect(lefts[1]).toBe(124)
  })

  it('pulls items back from the right edge when they crowd it', () => {
    const lefts = layoutRailItems([{ center: 280, width: 44 }, { center: 290, width: 44 }], 300)

    expect(lefts).toEqual([210, 256])
  })
})

describe('railPillsFit', () => {
  it('reports whether every item fits side by side', () => {
    expect(railPillsFit([96, 96, 44], 300)).toBe(true)
    expect(railPillsFit([96, 96, 96, 44], 300)).toBe(false)
  })
})
