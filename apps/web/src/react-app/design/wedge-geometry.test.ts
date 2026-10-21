import { fieldOfViewWedge, mountPointIn, wedgePolygonPoints } from './wedge-geometry'

const car = { x: 100, y: 200, width: 80, height: 160 }

describe('mountPointIn', () => {
  it('places a mount by fractions of the car image box', () => {
    expect(mountPointIn(car, [0.5, 0.22])).toEqual({ x: 140, y: 235.2 })
  })
})

describe('fieldOfViewWedge', () => {
  it('fans a front camera wedge to the bottom edge of a window above the car', () => {
    const wedge = fieldOfViewWedge({ x: 140, y: 235 }, { x: 60, y: 20, width: 160, height: 120 })

    expect(wedge.corners).toEqual([{ x: 74, y: 140 }, { x: 206, y: 140 }])
    expect(wedge.edgeMidpoint).toEqual({ x: 140, y: 140 })
  })

  it('fans a pillar wedge to the inner side of a window beside the car', () => {
    const wedge = fieldOfViewWedge({ x: 112, y: 273 }, { x: 0, y: 200, width: 60, height: 45 })

    expect(wedge.corners).toEqual([{ x: 60, y: 210 }, { x: 60, y: 235 }])
  })

  it('fans a rear wedge to the top edge of a window below the car', () => {
    const wedge = fieldOfViewWedge({ x: 140, y: 349 }, { x: 80, y: 400, width: 120, height: 90 })

    expect(wedge.corners).toEqual([{ x: 94, y: 400 }, { x: 186, y: 400 }])
    expect(wedgePolygonPoints(wedge)).toBe('140,349 94,400 186,400')
  })
})
