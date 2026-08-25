export type CompositeCanvasVisualTokens = {
  readonly canvas: string
  readonly cameraSurface: string
  readonly labelScrim: string
  readonly labelText: string
  readonly labelFont: string
  readonly labelInset: number
  readonly labelPaddingInline: number
  readonly labelHeight: number
  readonly labelBaseline: number
}

type ComputedStyle = Pick<CSSStyleDeclaration, 'getPropertyValue'>

const defaultTokens: CompositeCanvasVisualTokens = {
  canvas: '#000000',
  cameraSurface: '#000000',
  labelScrim: 'rgb(0 0 0 / 72%)',
  labelText: '#ffffff',
  labelFont: '500 13px -apple-system, BlinkMacSystemFont, "Hiragino Sans", "Yu Gothic UI", "Yu Gothic", sans-serif',
  labelInset: 8,
  labelPaddingInline: 8,
  labelHeight: 24,
  labelBaseline: 17,
}

function tokenValue(computedStyle: ComputedStyle, name: string, fallback: string): string {
  return computedStyle.getPropertyValue(name).trim() || fallback
}

function sizeTokenValue(computedStyle: ComputedStyle, name: string, fallback: number): number {
  const value = Number.parseFloat(computedStyle.getPropertyValue(name))
  return Number.isFinite(value) && value >= 0 ? value : fallback
}

export function readCompositeCanvasVisualTokens(computedStyle: ComputedStyle): CompositeCanvasVisualTokens {
  return {
    canvas: tokenValue(computedStyle, '--color-composite-canvas', defaultTokens.canvas),
    cameraSurface: tokenValue(computedStyle, '--color-composite-camera-surface', defaultTokens.cameraSurface),
    labelScrim: tokenValue(computedStyle, '--color-composite-label-scrim', defaultTokens.labelScrim),
    labelText: tokenValue(computedStyle, '--color-composite-label-text', defaultTokens.labelText),
    labelFont: tokenValue(computedStyle, '--font-composite-label', defaultTokens.labelFont),
    labelInset: sizeTokenValue(computedStyle, '--size-composite-label-inset', defaultTokens.labelInset),
    labelPaddingInline: sizeTokenValue(computedStyle, '--size-composite-label-padding-inline', defaultTokens.labelPaddingInline),
    labelHeight: sizeTokenValue(computedStyle, '--size-composite-label-height', defaultTokens.labelHeight),
    labelBaseline: sizeTokenValue(computedStyle, '--size-composite-label-baseline', defaultTokens.labelBaseline),
  }
}
