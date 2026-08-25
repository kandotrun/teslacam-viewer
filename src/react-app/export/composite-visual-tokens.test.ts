import { describe, expect, it } from 'vitest'

import {
  readCompositeCanvasVisualTokens,
  type CompositeCanvasVisualTokens,
} from './composite-visual-tokens'

const visualTokens: CompositeCanvasVisualTokens = {
  canvas: 'rgb(0, 0, 0)',
  cameraSurface: 'rgb(0, 0, 0)',
  labelScrim: 'rgb(0, 0, 0, 0.72)',
  labelText: 'rgb(255, 255, 255)',
  labelFont: '500 13px -apple-system, BlinkMacSystemFont, "Hiragino Sans", sans-serif',
  labelInset: 8,
  labelPaddingInline: 8,
  labelHeight: 24,
  labelBaseline: 17,
}

describe('readCompositeCanvasVisualTokens', () => {
  it('reads the composite label geometry from the supplied computed style', () => {
    const computedStyle = {
      getPropertyValue: (name: string) => ({
        '--size-composite-label-inset': '7px',
        '--size-composite-label-padding-inline': '9px',
        '--size-composite-label-height': '25px',
        '--size-composite-label-baseline': '18px',
      })[name] ?? '',
    }

    expect(readCompositeCanvasVisualTokens(computedStyle)).toMatchObject({
      labelInset: 7,
      labelPaddingInline: 9,
      labelHeight: 25,
      labelBaseline: 18,
    })
  })

  it('reads every composite Canvas token from the supplied computed style', () => {
    const computedStyle = {
      getPropertyValue: (name: string) => ({
        '--color-composite-canvas': visualTokens.canvas,
        '--color-composite-camera-surface': visualTokens.cameraSurface,
        '--color-composite-label-scrim': visualTokens.labelScrim,
        '--color-composite-label-text': visualTokens.labelText,
        '--font-composite-label': visualTokens.labelFont,
        '--size-composite-label-inset': `${visualTokens.labelInset}px`,
        '--size-composite-label-padding-inline': `${visualTokens.labelPaddingInline}px`,
        '--size-composite-label-height': `${visualTokens.labelHeight}px`,
        '--size-composite-label-baseline': `${visualTokens.labelBaseline}px`,
      })[name] ?? '',
    }

    expect(readCompositeCanvasVisualTokens(computedStyle)).toEqual(visualTokens)
  })

  it('falls back to neutral black surfaces and a compact Japanese system font', () => {
    const tokens = readCompositeCanvasVisualTokens({ getPropertyValue: () => '' })

    expect(tokens.canvas).toBe('#000000')
    expect(tokens.cameraSurface).toBe('#000000')
    expect(tokens.labelFont).toContain('500 13px')
    expect(tokens.labelFont).toContain('"Hiragino Sans"')
    expect(tokens.labelFont).not.toContain('Inter')
  })
})
