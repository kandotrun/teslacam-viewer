export type CompositeCanvasVisualTokens = {
  readonly canvas: string
  readonly cameraSurface: string
  readonly labelScrim: string
  readonly labelText: string
  readonly labelFont: string
}

type ComputedStyle = Pick<CSSStyleDeclaration, 'getPropertyValue'>

export function readCompositeCanvasVisualTokens(computedStyle: ComputedStyle): CompositeCanvasVisualTokens {
  return {
    canvas: computedStyle.getPropertyValue('--color-composite-canvas'),
    cameraSurface: computedStyle.getPropertyValue('--color-composite-camera-surface'),
    labelScrim: computedStyle.getPropertyValue('--color-composite-label-scrim'),
    labelText: computedStyle.getPropertyValue('--color-composite-label-text'),
    labelFont: computedStyle.getPropertyValue('--font-composite-label'),
  }
}
