import { describe, expect, it } from 'vitest'

import {
  readCompositeCanvasVisualTokens,
  type CompositeCanvasVisualTokens,
} from './composite-visual-tokens'

const visualTokens: CompositeCanvasVisualTokens = {
  canvas: 'rgb(8, 11, 16)',
  cameraSurface: 'rgb(17, 23, 32)',
  labelScrim: 'rgb(0, 0, 0, 0.72)',
  labelText: 'rgb(255, 255, 255)',
  labelFont: '600 18px Inter, sans-serif',
}

describe('readCompositeCanvasVisualTokens', () => {
  it('reads every composite Canvas token from the supplied computed style', () => {
    const computedStyle = {
      getPropertyValue: (name: string) => ({
        '--color-composite-canvas': visualTokens.canvas,
        '--color-composite-camera-surface': visualTokens.cameraSurface,
        '--color-composite-label-scrim': visualTokens.labelScrim,
        '--color-composite-label-text': visualTokens.labelText,
        '--font-composite-label': visualTokens.labelFont,
      })[name] ?? '',
    }

    expect(readCompositeCanvasVisualTokens(computedStyle)).toEqual(visualTokens)
  })
})
