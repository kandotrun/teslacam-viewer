import { readFile } from 'node:fs/promises'

import { expect, test } from '@playwright/test'

import { fixturePath, installPicker, selectFolder, textSpansSingleLine } from './support/viewer-fixtures'

test.describe('rich investigation layout', () => {
  test('keeps desktop investigation regions and evidence inside 1280 by 1000', async ({ browser }) => {
    const media = [...await readFile(fixturePath)]
    const context = await browser.newContext({ viewport: { width: 1280, height: 1000 } })
    await installPicker(context, media, 'full')
    const page = await context.newPage()
    await page.goto('/')
    await selectFolder(page)
    await page.getByRole('button', { name: /2026-08-20 20:35:00/ }).click()

    const regions = await page
      .locator('.event-navigator, .camera-stage, .event-inspector, .playback-deck')
      .evaluateAll((elements) => elements.map((element) => element.getBoundingClientRect()))
    expect(regions).toHaveLength(4)
    expect(regions.every((bounds) => (
      bounds.left >= 0
      && bounds.right <= 1280
      && bounds.top >= 0
      && bounds.bottom <= 1000
    ))).toBe(true)
    expect(await textSpansSingleLine(
      page.getByText('セントリー', { exact: true }).last(),
      'セントリー',
    )).toBe(true)
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
    expect(await page.locator('.camera-tile video').evaluateAll((videos) => (
      videos.every((video) => getComputedStyle(video).objectFit === 'contain')
    ))).toBe(true)
    await context.close()
  })

  test('uses a three by two camera grid and removes the inspector from the tablet stage', async ({ browser }) => {
    const media = [...await readFile(fixturePath)]
    const context = await browser.newContext({ viewport: { width: 768, height: 1024 } })
    await installPicker(context, media, 'full')
    const page = await context.newPage()
    await page.goto('/')
    await selectFolder(page)
    await page.getByRole('button', { name: /2026-08-20 20:35:00/ }).click()

    const tileBounds = await page.locator('.camera-tile').evaluateAll((elements) => elements.map((element) => {
      const bounds = element.getBoundingClientRect()
      return { left: Math.round(bounds.left), top: Math.round(bounds.top) }
    }))
    expect(tileBounds).toHaveLength(6)
    expect(new Set(tileBounds.map((bounds) => bounds.left)).size).toBe(3)
    expect(new Set(tileBounds.map((bounds) => bounds.top)).size).toBe(2)
    const cameraBottom = await page.locator('.camera-grid').evaluate((element) => element.getBoundingClientRect().bottom)
    const playbackTop = await page.locator('.playback-deck').evaluate((element) => element.getBoundingClientRect().top)
    expect(playbackTop - cameraBottom).toBeGreaterThanOrEqual(0)
    expect(playbackTop - cameraBottom).toBeLessThanOrEqual(1)
    await expect(page.locator('.event-inspector')).toBeHidden()
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
    await context.close()
  })

  test('keeps one mobile camera visible and localizes horizontal scrolling to its switcher', async ({ browser }) => {
    const media = [...await readFile(fixturePath)]
    const context = await browser.newContext({ viewport: { width: 375, height: 844 } })
    await installPicker(context, media, 'full')
    const page = await context.newPage()
    await page.goto('/')
    await selectFolder(page)
    await page.getByRole('button', { name: /2026-08-20 20:35:00/ }).click()

    const switcher = page.locator('.camera-switcher')
    await expect(switcher.getByRole('button')).toHaveCount(6)
    await expect(page.locator('.camera-tile:visible')).toHaveCount(1)
    expect(await switcher.evaluate((element) => element.scrollWidth > element.clientWidth)).toBe(true)
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
    const outsideScrollers = await page.locator('body *').evaluateAll((elements) => elements.filter((element) => (
      element.closest('.camera-switcher') === null
      && element.scrollWidth > element.clientWidth + 1
      && getComputedStyle(element).overflowX !== 'hidden'
    )).map((element) => element.className))
    expect(outsideScrollers).toEqual([])
    await context.close()
  })
})
