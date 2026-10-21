import tokenStylesheet from './tokens.css?raw'
import { contrastRatio } from '../design/color-contrast'
import { colorToken, designTokens } from '../design/contract'
import { designTokenCustomProperties, parseRootCustomProperties, renderTokenStylesheet } from '../design/token-css'

function normalized(value: string): string {
  return value.replace(/#[0-9a-f]{6,8}\b/giu, (hex) => hex.toUpperCase())
}

describe('tokens.css', () => {
  it('defines exactly the custom properties of design-tokens.json', () => {
    const expected = designTokenCustomProperties(designTokens)
    const actual = parseRootCustomProperties(tokenStylesheet)

    const drift = [...new Set([...expected.keys(), ...actual.keys()])].flatMap((name) => {
      const want = expected.get(name)
      const have = actual.get(name)
      return want !== undefined && have !== undefined && normalized(want) === normalized(have)
        ? []
        : [{ name, want: want ?? null, have: have ?? null }]
    })

    expect(drift, `Regenerate tokens.css:\n${renderTokenStylesheet(designTokens)}`).toEqual([])
  })
})

describe('design token contrast', () => {
  it.each(designTokens.contrast)('$fg on $bg reaches $min:1', ({ fg, bg, min }) => {
    expect(contrastRatio(colorToken(fg), colorToken(bg), colorToken('surface'))).toBeGreaterThanOrEqual(min)
  })
})
