import { describe, expect, it } from 'vitest'

import { buildCompositeLayout, containRect, drawCompositeFrame } from './composite-layout'
import type { CompositeCanvasVisualTokens } from './composite-visual-tokens'

const visualTokens: CompositeCanvasVisualTokens = {
  canvas: 'rgb(8, 11, 16)',
  cameraSurface: 'rgb(17, 23, 32)',
  labelScrim: 'rgb(0, 0, 0, 0.72)',
  labelText: 'rgb(255, 255, 255)',
  labelFont: '600 18px Inter, sans-serif',
}

describe('buildCompositeLayout', () => {
  it('places the six canonical cameras around the vehicle within 1280x720', () => {
    const layout = buildCompositeLayout()

    expect(layout).toMatchObject({ width: 1280, height: 720 })
    expect(layout.cameras.map(({ cameraId }) => cameraId)).toEqual([
      'front',
      'left_pillar',
      'right_pillar',
      'left_repeater',
      'back',
      'right_repeater',
    ])
    expect(layout.cameras.every(({ frame }) =>
      frame.x >= 0 && frame.y >= 0 &&
      frame.x + frame.width <= layout.width &&
      frame.y + frame.height <= layout.height,
    )).toBe(true)
    expect(layout.cameras.find(({ cameraId }) => cameraId === 'front')?.frame.y).toBeLessThan(layout.vehicle.y)
    expect(layout.cameras.find(({ cameraId }) => cameraId === 'back')?.frame.y).toBeGreaterThan(layout.vehicle.y)
    expect(layout.cameras.find(({ cameraId }) => cameraId === 'left_pillar')?.frame.x).toBeLessThan(layout.vehicle.x)
    expect(layout.cameras.find(({ cameraId }) => cameraId === 'right_pillar')?.frame.x).toBeGreaterThan(layout.vehicle.x)
  })

  it('contains a source without cropping it', () => {
    expect(containRect({ width: 1920, height: 1080 }, { x: 0, y: 0, width: 300, height: 300 })).toEqual({
      x: 0,
      y: 65.625,
      width: 300,
      height: 168.75,
    })
  })

  it('draws the transparent vehicle asset contained at its natural 1:2 ratio', () => {
    const layout = buildCompositeLayout()
    const vehicle = document.createElement('img')
    Object.defineProperties(vehicle, {
      naturalWidth: { value: 360 },
      naturalHeight: { value: 720 },
    })
    const calls: unknown[][] = []
    const canvas = document.createElement('canvas')
    Object.defineProperty(canvas, 'getContext', {
      value: () => ({
        drawImage: (...arguments_: [CanvasImageSource, number, number, number, number]) => calls.push(arguments_),
        fillRect: () => undefined,
        fillText: () => undefined,
      }),
    })
    const context = canvas.getContext('2d')
    if (context === null) throw new Error('canvas context is missing')

    drawCompositeFrame(context, layout, [], vehicle, visualTokens)

    expect(calls).toContainEqual([vehicle, 594, 260, 92, 184])
  })
})
