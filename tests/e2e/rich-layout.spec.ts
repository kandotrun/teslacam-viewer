import { readFile } from 'node:fs/promises'

import { expect, test } from '@playwright/test'

import { evidenceRoot, fixturePath, installPicker, selectFolder, textSpansSingleLine } from './support/viewer-fixtures'

test.describe('rich investigation layout', () => {
  test('keeps desktop investigation regions and evidence inside 1280 by 1000', async ({ browser }) => {
    const media = [...await readFile(fixturePath)]
    const context = await browser.newContext({ viewport: { width: 1280, height: 1000 } })
    await installPicker(context, media, 'full')
    const page = await context.newPage()
    await page.goto('/')
    await selectFolder(page)
    await page.getByRole('button', { name: /2026-08-20 20:35:00/ }).click()
    await expect(page.locator('.camera-tile[data-status="ready"]')).toHaveCount(6)
    await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
    await page.screenshot({ path: `${evidenceRoot}/task-7/1280-selected-sentry.png` })

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
    const dayTimelineGeometry = await page.locator('.day-timeline__marker:visible, .day-timeline__summary *:visible').evaluateAll((elements) => {
      const timeline = document.querySelector('.day-timeline')
      if (timeline === null) throw new Error('day timeline is missing')
      const container = timeline.getBoundingClientRect()
      return elements.map((element) => {
        const bounds = element.getBoundingClientRect()
        return {
          label: element.textContent,
          bounds: { left: bounds.left, right: bounds.right, top: bounds.top, bottom: bounds.bottom },
          container: { left: container.left, right: container.right, top: container.top, bottom: container.bottom },
          viewport: { width: window.innerWidth, height: window.innerHeight },
        }
      })
    })
    expect(dayTimelineGeometry.every(({ bounds, container, viewport }) => (
      bounds.left >= container.left
      && bounds.right <= container.right
      && bounds.top >= container.top
      && bounds.bottom <= container.bottom
      && bounds.left >= 0
      && bounds.right <= viewport.width
      && bounds.top >= 0
      && bounds.bottom <= viewport.height
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

  test('uses a three by two camera grid and keeps details available as a tablet disclosure', async ({ browser }) => {
    const media = [...await readFile(fixturePath)]
    const context = await browser.newContext({ viewport: { width: 768, height: 1024 } })
    await installPicker(context, media, 'full')
    const page = await context.newPage()
    await page.goto('/')
    await selectFolder(page)
    await page.getByRole('button', { name: /2026-08-20 20:35:00/ }).click()
    await expect(page.locator('.camera-tile[data-status="ready"]')).toHaveCount(6)
    await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
    await page.screenshot({ path: `${evidenceRoot}/task-7/768-selected-sentry.png` })

    const tileBounds = await page.locator('.camera-tile').evaluateAll((elements) => elements.map((element) => {
      const bounds = element.getBoundingClientRect()
      return { left: Math.round(bounds.left), top: Math.round(bounds.top) }
    }))
    expect(tileBounds).toHaveLength(6)
    expect(new Set(tileBounds.map((bounds) => bounds.left)).size).toBe(3)
    expect(new Set(tileBounds.map((bounds) => bounds.top)).size).toBe(2)
    const cameraBottom = await page.locator('.camera-grid').evaluate((element) => element.getBoundingClientRect().bottom)
    const inspector = page.locator('.event-inspector')
    await expect(inspector).toBeVisible()
    await expect(inspector).not.toHaveAttribute('open')
    const inspectorBounds = await inspector.evaluate((element) => element.getBoundingClientRect())
    const playbackTop = await page.locator('.playback-deck').evaluate((element) => element.getBoundingClientRect().top)
    expect(inspectorBounds.top).toBeGreaterThanOrEqual(cameraBottom)
    expect(playbackTop).toBeGreaterThanOrEqual(inspectorBounds.bottom)
    await inspector.locator('summary').click()
    await expect(inspector).toHaveAttribute('open')
    await page.screenshot({ path: `${evidenceRoot}/task-7/768-event-details-open.png` })
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
    await expect(page.locator('.camera-tile[data-status="ready"]')).toHaveCount(6)
    await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
    expect(await textSpansSingleLine(page.getByRole('button', { name: '一時停止' }), '一時停止')).toBe(true)
    await page.screenshot({ path: `${evidenceRoot}/task-7/375-selected-sentry.png` })

    const switcher = page.locator('.camera-switcher')
    await expect(switcher.getByRole('button')).toHaveCount(6)
    await expect(page.locator('.camera-tile:visible')).toHaveCount(1)
    await expect(page.locator('.event-inspector')).toBeHidden()
    const timelineSummary = page.locator('.day-timeline__summary')
    await expect(timelineSummary.getByLabel('前のイベント')).toBeVisible()
    await expect(timelineSummary.getByLabel('選択中のイベント')).toBeVisible()
    await expect(timelineSummary.getByLabel('次のイベント')).toBeVisible()
    await expect(timelineSummary.getByLabel('この日のイベント数')).toHaveText(/同日: 3件/)
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

  test('preserves the six synchronized video nodes through spatial, grid and expanded states', async ({ browser }) => {
    const media = [...await readFile(fixturePath)]
    const context = await browser.newContext({ viewport: { width: 1280, height: 1000 } })
    await installPicker(context, media, 'full')
    const page = await context.newPage()
    await page.goto('/')
    await selectFolder(page)
    await page.getByRole('button', { name: /2026-08-20 20:35:00/ }).click()
    const videos = page.locator('.camera-tile video')
    await expect(videos).toHaveCount(6)
    await expect(page.locator('.camera-tile[data-status="ready"]')).toHaveCount(6)
    await videos.evaluateAll((elements) => elements.forEach((element, index) => {
      element.dataset['taskSevenNode'] = String(index)
    }))
    await page.screenshot({ path: `${evidenceRoot}/task-7/1280-spatial-sentry.png` })
    await page.getByRole('button', { name: '6分割', exact: true }).click()
    await expect(page.locator('.camera-stage')).toHaveAttribute('data-view-mode', 'grid')
    await page.screenshot({ path: `${evidenceRoot}/task-7/1280-grid-sentry.png` })
    await page.getByRole('button', { name: 'フロントカメラを拡大', exact: true }).click()
    await expect(page.locator('.camera-stage')).toHaveAttribute('data-expanded', 'true')
    await page.screenshot({ path: `${evidenceRoot}/task-7/1280-expanded-sentry.png` })
    await page.keyboard.press('Escape')
    await expect(page.locator('.camera-stage')).toHaveAttribute('data-expanded', 'false')
    expect(await videos.evaluateAll((elements) => elements.map((element) => element.dataset['taskSevenNode']))).toEqual(['0', '1', '2', '3', '4', '5'])
    await context.close()
  })
})
