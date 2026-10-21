/**
 * The CSS custom properties that `styles/tokens.css` must define for `design-tokens.json`. The token test
 * compares the stylesheet with this mapping, so the stylesheet cannot drift from the contract.
 */
import type { DesignTokens } from './contract'

function formatNumber(value: number): string {
  return String(Number(value.toFixed(4)))
}

function shadowValue(layers: DesignTokens['shadow'][string]): string {
  return layers.map((layer) => `${formatNumber(layer.x)}px ${formatNumber(layer.y)}px ${formatNumber(layer.blur)}px ${layer.color}`).join(', ')
}

function fontFamily(names: readonly string[]): string {
  return names.map((name) => (/^[a-z-]+$/u.test(name) ? name : `"${name}"`)).join(', ')
}

function easing([x1, y1, x2, y2]: readonly [number, number, number, number]): string {
  return `cubic-bezier(${[x1, y1, x2, y2].map(formatNumber).join(', ')})`
}

export function designTokenCustomProperties(tokens: DesignTokens): ReadonlyMap<string, string> {
  const properties = new Map<string, string>()
  for (const [name, value] of Object.entries(tokens.color)) properties.set(`--${name}`, value)
  for (const [name, layers] of Object.entries(tokens.shadow)) properties.set(`--shadow-${name}`, shadowValue(layers))
  for (const [name, value] of Object.entries(tokens.radius)) properties.set(`--radius-${name}`, `${formatNumber(value)}px`)
  properties.set('--font-ui', fontFamily(tokens.font.ui))
  properties.set('--font-num', fontFamily(tokens.font.num))
  for (const [name, role] of Object.entries(tokens.type)) {
    properties.set(`--type-${name}`, `${role.weight} ${formatNumber(role.size)}px/${formatNumber(role.lineHeight)} var(--font-${role.family})`)
  }
  properties.set('--space-unit', `${formatNumber(tokens.space.unit)}px`)
  properties.set('--gutter', `${formatNumber(tokens.space.gutter)}px`)
  for (const [name, value] of Object.entries(tokens.size)) properties.set(`--size-${name}`, `${formatNumber(value)}px`)
  for (const name of ['fast', 'sheet'] as const) {
    properties.set(`--motion-${name}-duration`, `${formatNumber(tokens.motion[name].durationMs)}ms`)
    properties.set(`--motion-${name}-easing`, easing(tokens.motion[name].easing))
  }
  properties.set('--motion-reveal-stagger', `${formatNumber(tokens.motion['reveal-stagger-ms'])}ms`)
  properties.set('--stage-rows', tokens.stage.rows.map((row) => `minmax(0, ${formatNumber(row)}fr)`).join(' '))
  properties.set('--stage-columns', tokens.stage.columns.map((column) => `minmax(0, ${formatNumber(column)}fr)`).join(' '))
  properties.set('--stage-gap-row', `${formatNumber(tokens.stage.gap.row)}px`)
  properties.set('--stage-gap-column', `${formatNumber(tokens.stage.gap.column)}px`)
  properties.set('--stage-back-width', `${formatNumber(tokens.stage['back-width-ratio'] * 100)}%`)
  return properties
}

/** Custom properties declared in a stylesheet's `:root` rule, with whitespace normalised. */
export function parseRootCustomProperties(css: string): ReadonlyMap<string, string> {
  const root = /:root\s*\{([^}]*)\}/u.exec(css)
  const properties = new Map<string, string>()
  if (root === null) return properties
  for (const match of (root[1] ?? '').matchAll(/(--[a-z0-9-]+)\s*:\s*([^;]+);/giu)) {
    const [, name, value] = match
    if (name !== undefined && value !== undefined) properties.set(name, value.trim().replace(/\s+/gu, ' '))
  }
  return properties
}

export function renderTokenStylesheet(tokens: DesignTokens): string {
  const lines = [...designTokenCustomProperties(tokens)].map(([name, value]) => `  ${name}: ${value};`)
  return [
    '/* Generated from packages/contract/design-tokens.json; tokens.test.ts fails when they differ. */',
    ':root {',
    ...lines,
    '}',
    '',
  ].join('\n')
}
