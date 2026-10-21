import compositeLayoutJson from '../../../../../packages/contract/composite-layout.json'
import {
  buildCompositeLayout,
  compositeLabelGeometry,
  compositeOverlayKey,
  compositeSlotFrames,
  containRect,
} from './composite-layout'

describe('buildCompositeLayout', () => {
  it('uses the slot rectangles of composite-layout.json in draw order', () => {
    const layout = buildCompositeLayout()

    expect(layout).toMatchObject({ width: 1280, height: 720, fps: 30, vehicle: { x: 598, y: 285, width: 84, height: 168 } })
    expect(layout.cameras.map(({ cameraId, label, frame }) => [cameraId, label, frame])).toEqual([
      ['front', 'フロント', compositeLayoutJson.slots.front],
      ['left_pillar', '左ピラー', compositeLayoutJson.slots.left_pillar],
      ['right_pillar', '右ピラー', compositeLayoutJson.slots.right_pillar],
      ['left_repeater', '左リピーター', compositeLayoutJson.slots.left_repeater],
      ['back', 'リア', compositeLayoutJson.slots.back],
      ['right_repeater', '右リピーター', compositeLayoutJson.slots.right_repeater],
    ])
  })
})

describe('containRect', () => {
  it('contains a source without cropping it', () => {
    expect(containRect({ width: 1920, height: 1080 }, { x: 0, y: 0, width: 300, height: 300 })).toEqual({
      x: 0,
      y: 65.625,
      width: 300,
      height: 168.75,
    })
  })

  it('fits the 1:2 car image exactly into the vehicle box', () => {
    expect(containRect({ width: 360, height: 720 }, buildCompositeLayout().vehicle)).toEqual({ x: 598, y: 285, width: 84, height: 168 })
  })

  it('uses the destination for an unknown source size', () => {
    expect(containRect({ width: 0, height: 0 }, { x: 1, y: 2, width: 3, height: 4 })).toEqual({ x: 1, y: 2, width: 3, height: 4 })
  })
})

describe('compositeLabelGeometry', () => {
  it('puts the name pill at the bottom-left of the visible video', () => {
    const label = compositeLabelGeometry('リア', { x: 480, y: 456, width: 320, height: 240 }, 26.4)

    expect(label.pill).toEqual({ x: 490, y: 662, width: 47, height: 24 })
    expect(label.radius).toBe(12)
    expect(label.textX).toBe(500)
    expect(label.textY).toBeCloseTo(678.68)
  })
})

describe('compositeSlotFrames', () => {
  it('fits each camera into its slot and shapes missing tiles like the others', () => {
    const slots = compositeSlotFrames(buildCompositeLayout(), { front: { width: 1448, height: 938 } }, { width: 1448, height: 938 })
    const front = slots.find((slot) => slot.cameraId === 'front')
    const back = slots.find((slot) => slot.cameraId === 'back')

    expect(front?.playable).toBe(true)
    expect(front?.videoRect.width).toBe(352)
    expect(front?.videoRect.height).toBeCloseTo(228.02, 2)
    expect(back?.playable).toBe(false)
    expect(back?.videoRect.width).toBe(320)
    expect(back?.videoRect.height).toBeCloseTo(207.29, 1)
  })

  it('aims each wedge from the camera mount on the car to the window edge that faces it', () => {
    const slots = compositeSlotFrames(buildCompositeLayout(), {}, null)
    const front = slots.find((slot) => slot.cameraId === 'front')
    const leftPillar = slots.find((slot) => slot.cameraId === 'left_pillar')

    expect(front?.wedge.mount).toEqual({ x: 640, y: 285 + 168 * 0.22 })
    expect(front?.wedge.corners.map((corner) => corner.y)).toEqual([276, 276])
    expect(leftPillar?.wedge.corners.map((corner) => corner.x)).toEqual([408, 408])
  })

  it('keys overlays by arrangement so identical frames reuse one overlay', () => {
    const layout = buildCompositeLayout()
    const sizes = { front: { width: 1448, height: 938 } }
    const first = compositeOverlayKey(compositeSlotFrames(layout, sizes, null))
    const same = compositeOverlayKey(compositeSlotFrames(layout, sizes, null))
    const different = compositeOverlayKey(compositeSlotFrames(layout, {}, null))

    expect(same).toBe(first)
    expect(different).not.toBe(first)
  })
})
