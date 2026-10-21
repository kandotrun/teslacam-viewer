/** WCAG 2.2 contrast for `#RRGGBB` / `#RRGGBBAA` token values. */

export type Rgba = { readonly red: number; readonly green: number; readonly blue: number; readonly alpha: number }

export function parseHexColor(value: string): Rgba {
  const match = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})?$/iu.exec(value)
  if (match === null) throw new Error(`Not a hex colour: ${value}`)
  const channel = (hex: string | undefined, fallback: number): number => (hex === undefined ? fallback : Number.parseInt(hex, 16))
  return {
    red: channel(match[1], 0),
    green: channel(match[2], 0),
    blue: channel(match[3], 0),
    alpha: channel(match[4], 255) / 255,
  }
}

/** Composites a possibly translucent colour over an opaque backdrop. */
export function compositeOver(color: Rgba, backdrop: Rgba): Rgba {
  const mix = (front: number, back: number): number => front * color.alpha + back * (1 - color.alpha)
  return {
    red: mix(color.red, backdrop.red),
    green: mix(color.green, backdrop.green),
    blue: mix(color.blue, backdrop.blue),
    alpha: 1,
  }
}

export function relativeLuminance(color: Rgba): number {
  const linear = (channel: number): number => {
    const value = channel / 255
    return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4
  }
  return 0.2126 * linear(color.red) + 0.7152 * linear(color.green) + 0.0722 * linear(color.blue)
}

/**
 * Contrast of `foreground` text on `background`. Translucent colours are composited first: the
 * background over `backdrop`, then the foreground over the result.
 */
export function contrastRatio(foreground: string, background: string, backdrop: string): number {
  const opaqueBackground = compositeOver(parseHexColor(background), parseHexColor(backdrop))
  const opaqueForeground = compositeOver(parseHexColor(foreground), opaqueBackground)
  const [lighter, darker] = [relativeLuminance(opaqueForeground), relativeLuminance(opaqueBackground)]
    .sort((left, right) => right - left)
  return ((lighter ?? 0) + 0.05) / ((darker ?? 0) + 0.05)
}
