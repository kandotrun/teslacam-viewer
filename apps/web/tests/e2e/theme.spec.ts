import { expect, test } from '@playwright/test'

import { contrastRatio, relativeLuminance } from './support/viewer-fixtures'

test('defaults browser, PWA and workspace surfaces to light', async ({ page }) => {
  await page.goto('/')
  const theme = await page.evaluate(async () => {
    const rootStyles = getComputedStyle(document.documentElement)
    const bodyStyles = getComputedStyle(document.body)
    const themeColor = document.querySelector<HTMLMetaElement>('meta[name="theme-color"]')?.content
    const response = await fetch('/manifest.webmanifest')
    const manifest: unknown = await response.json()
    if (
      typeof manifest !== 'object'
      || manifest === null
      || !('background_color' in manifest)
      || typeof manifest.background_color !== 'string'
      || !('theme_color' in manifest)
      || typeof manifest.theme_color !== 'string'
    ) {
      throw new Error('invalid web app manifest theme')
    }
    return {
      scheme: rootStyles.colorScheme,
      background: bodyStyles.backgroundColor,
      text: bodyStyles.color,
      browser: themeColor,
      pwaBackground: manifest.background_color,
      pwaTheme: manifest.theme_color,
    }
  })

  expect(theme.scheme).toBe('light')
  expect(relativeLuminance(theme.background)).toBeGreaterThan(0.8)
  expect(contrastRatio(theme.text, theme.background)).toBeGreaterThanOrEqual(4.5)
  if (theme.browser === undefined) throw new Error('browser theme color is missing')
  expect(relativeLuminance(theme.browser)).toBeGreaterThan(0.8)
  expect(relativeLuminance(theme.pwaBackground)).toBeGreaterThan(0.8)
  expect(relativeLuminance(theme.pwaTheme)).toBeGreaterThan(0.8)
})
