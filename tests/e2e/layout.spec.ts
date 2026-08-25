import { readFile } from 'node:fs/promises'

import { expect, test } from '@playwright/test'

import { captureRuntimeErrors, contrastRatio, evidenceRoot, fixturePath, installPicker, relativeLuminance, selectFolder, textSpansSingleLine, viewports } from './support/viewer-fixtures'

test('captures welcome and loaded states at the required viewports', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  for (const viewport of viewports) {
    const context = await browser.newContext({ viewport: { width: viewport.width, height: viewport.height } })
    await installPicker(context, media, 'full')
    const page = await context.newPage()
    const runtimeErrors = captureRuntimeErrors(page)
    await page.goto('/')
    await expect(page.getByRole('heading', { name: 'SSDのTeslaCamフォルダを開く' })).toBeVisible()
    await expect(page.getByText('選ぶもの', { exact: true })).toBeVisible()
    await expect(page.getByText('表示されるもの', { exact: true })).toBeVisible()
    expect(await textSpansSingleLine(page.getByRole('heading', { name: 'SSDのTeslaCamフォルダを開く' }), 'フォルダを開く')).toBe(true)
    const welcomeColors = await page.locator('body').evaluate((element) => {
      const styles = getComputedStyle(element)
      return { background: styles.backgroundColor, text: styles.color }
    })
    expect(relativeLuminance(welcomeColors.background)).toBeGreaterThan(0.8)
    expect(contrastRatio(welcomeColors.text, welcomeColors.background)).toBeGreaterThanOrEqual(4.5)
    if (viewport.mode === 'mobile') {
      await page.setViewportSize({ width: 320, height: 844 })
      const narrowAction = await page.getByRole('button', { name: 'TeslaCamフォルダを選ぶ', exact: true }).evaluate((element) => {
        const labelNode = [...element.childNodes].find((node) => node.nodeType === Node.TEXT_NODE && node.textContent?.trim() !== '')
        const range = document.createRange()
        if (labelNode !== undefined) range.selectNodeContents(labelNode)
        return {
          labelLineCount: labelNode === undefined ? 0 : range.getClientRects().length,
          height: element.getBoundingClientRect().height,
          overflowFree: document.documentElement.scrollWidth <= window.innerWidth,
        }
      })
      expect(narrowAction.labelLineCount).toBe(1)
      expect(narrowAction.height).toBeGreaterThanOrEqual(44)
      expect(narrowAction.height).toBeLessThanOrEqual(48)
      expect(narrowAction.overflowFree).toBe(true)
      await page.setViewportSize({ width: viewport.width, height: viewport.height })
    }
    await page.screenshot({ path: `${evidenceRoot}/${viewport.name}-welcome.png` })
    const selection = page.getByRole('button', { name: 'TeslaCamフォルダを選ぶ', exact: true })
    await selection.focus()
    await expect(selection).toBeFocused()
    await selection.click()
    await expect(page.getByText(/250 ファイルを確認中です/)).toBeVisible()
    await expect(page.getByRole('button', { name: '別のフォルダを選ぶ' })).toBeVisible()
    expect(await page.locator('.app-header').evaluate((element) => element.getBoundingClientRect().height)).toBeLessThanOrEqual(64)
    if (viewport.mode === 'tablet') {
      await expect(page.getByRole('button', { name: 'イベント一覧を閉じる' })).toBeVisible()
    }
    await expect(page.locator('.event-thumbnail')).toHaveCount(0)
    await page.getByRole('button', { name: /2026-08-20 20:45:00/ }).click()
    await expect(page.locator('video')).toHaveCount(6)
    const evidenceContext = page.getByLabel('再生中のイベント')
    await expect(page.getByText('TeslaCam', { exact: true })).toHaveCount(1)
    await expect(evidenceContext).toContainText('2026-08-20 20:45:00')
    await expect(evidenceContext).toContainText('6/6 カメラ')
    await expect(evidenceContext.locator('.theater-context__actions')).toBeVisible()
    const contextBounds = await evidenceContext.boundingBox()
    const exportBounds = await page.getByRole('button', { name: '合成動画を書き出す' }).boundingBox()
    if (contextBounds === null || exportBounds === null) throw new Error('compact theater context geometry is missing')
    expect(exportBounds.y).toBeGreaterThanOrEqual(contextBounds.y)
    expect(exportBounds.y + exportBounds.height).toBeLessThanOrEqual(contextBounds.y + contextBounds.height)
    const scanWarnings = page.locator('.scan-warnings')
    await expect(scanWarnings).not.toHaveAttribute('open')
    await expect(scanWarnings.locator('summary')).toBeVisible()
    const surfaceColors = await page.locator('.library-sidebar, .event-list-panel, .theater-context, .camera-switcher, .playback-controls').evaluateAll((elements) => elements.map((element) => getComputedStyle(element).backgroundColor))
    expect(surfaceColors.every((color) => relativeLuminance(color) > 0.75)).toBe(true)
    const theaterColors = await page.locator('.camera-grid, .camera-tile').evaluateAll((elements) => elements.map((element) => getComputedStyle(element).backgroundColor))
    expect(theaterColors.every((color) => relativeLuminance(color) < 0.05)).toBe(true)
    await expect.poll(async () => {
      const interactiveBoundaries = await page.locator('.camera-tile[data-active="true"], .filter-field input, .category-action[data-active="true"], .event-card[data-selected="true"], .quiet-action, .playback-controls button:not(:disabled), .camera-switcher button[aria-pressed="true"]:visible, .event-list-disclosure:visible').evaluateAll((elements) => elements.map((element) => {
        const styles = getComputedStyle(element)
        return { name: element.getAttribute('aria-label') ?? element.getAttribute('class') ?? element.tagName, border: styles.borderTopColor, fill: styles.backgroundColor }
      }))
      return interactiveBoundaries.filter((colors) => contrastRatio(colors.border, colors.fill) < 3)
    }).toEqual([])
    if (viewport.mode !== 'desktop') {
      await expect(page.getByRole('button', { name: 'イベント一覧を開く' })).toHaveAttribute('aria-expanded', 'false')
      const activeCamera = await page.locator('.camera-tile[data-active="true"]').boundingBox()
      const transport = await page.locator('.playback-controls').boundingBox()
      if (activeCamera === null || transport === null) throw new Error('responsive theater geometry is missing')
      expect(activeCamera.y + activeCamera.height).toBeLessThanOrEqual(transport.y)
      if (viewport.mode === 'mobile') {
        expect(activeCamera.height).toBeGreaterThanOrEqual(viewport.height * 0.45)
        expect(transport.height).toBeLessThanOrEqual(140)
      }
    }
    if (viewport.mode === 'tablet') {
      const maximumControlHeight = await page.locator('.playback-controls__primary button').evaluateAll((elements) => Math.max(...elements.map((element) => element.getBoundingClientRect().height)))
      expect(maximumControlHeight).toBeLessThanOrEqual(48)
    }
    await page.screenshot({ path: `${evidenceRoot}/${viewport.name}-loaded.png` })
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
    expect(runtimeErrors).toEqual([])
    await context.close()
  }
})

test('shows every camera frame without cropping at the required viewports', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  for (const viewport of viewports) {
    const context = await browser.newContext({ viewport: { width: viewport.width, height: viewport.height } })
    await installPicker(context, media, 'full')
    const page = await context.newPage()
    await page.goto('/')
    await selectFolder(page)
    await page.getByRole('button', { name: /2026-08-20 20:45:00/ }).click()
    await expect(page.locator('.camera-tile[data-status="ready"]')).toHaveCount(6)

    const frames = await page.locator('.camera-tile video').evaluateAll((videos) => videos.map((video) => {
      if (!(video instanceof HTMLVideoElement)) throw new Error('camera frame is not a video')
      return {
        fit: getComputedStyle(video).objectFit,
        mediaWidth: video.videoWidth,
        mediaHeight: video.videoHeight,
      }
    }))

    expect(frames.every((frame) => frame.fit === 'contain')).toBe(true)
    expect(frames.every((frame) => frame.mediaWidth > 0 && frame.mediaHeight > 0)).toBe(true)
    await context.close()
  }
})

test('keeps the loaded product header on one line at 320px', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  const context = await browser.newContext({ viewport: { width: 320, height: 844 } })
  await installPicker(context, media, 'full')
  const page = await context.newPage()
  await page.goto('/')
  await selectFolder(page)

  const header = page.locator('.app-header')
  const identity = header.locator('.app-header__identity')
  const action = header.getByRole('button', { name: '別のフォルダを選ぶ' })
  const geometry = await Promise.all([identity.boundingBox(), action.boundingBox()])
  const identityBounds = geometry.at(0)
  const actionBounds = geometry.at(1)
  if (identityBounds === null || actionBounds === null) throw new Error('compact app header geometry is missing')
  const identityCenter = identityBounds.y + (identityBounds.height / 2)
  const actionCenter = actionBounds.y + (actionBounds.height / 2)
  expect(Math.abs(identityCenter - actionCenter)).toBeLessThanOrEqual(1)
  expect(identityBounds.x + identityBounds.width).toBeLessThanOrEqual(actionBounds.x)
  expect(await header.evaluate((element) => element.scrollWidth <= element.clientWidth)).toBe(true)
  expect(await textSpansSingleLine(action, 'フォルダを選ぶ')).toBe(true)
  await context.close()
})

test('keeps responsive disclosures, mobile cameras, reduced motion and expanded evidence accurate', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  const context = await browser.newContext({ viewport: { width: 375, height: 844 }, reducedMotion: 'reduce' })
  await installPicker(context, media, 'full')
  const page = await context.newPage()
  await page.goto('/')
  await selectFolder(page)
  const disclosure = page.getByRole('button', { name: 'イベント一覧を閉じる' })
  await expect(disclosure).toBeVisible()
  await disclosure.click()
  await expect(page.getByRole('button', { name: 'イベント一覧を開く' })).toHaveAttribute('aria-expanded', 'false')
  await page.getByRole('button', { name: 'イベント一覧を開く' }).click()
  await page.getByRole('button', { name: /2026-08-20 20:45:00/ }).click()
  await expect(page.getByRole('group', { name: '表示するカメラ' })).toBeVisible()
  const switcher = page.getByRole('group', { name: '表示するカメラ' })
  await expect(switcher.getByRole('button')).toHaveCount(6)
  expect(await switcher.evaluate((element) => element.scrollWidth > element.clientWidth)).toBe(true)
  await switcher.getByRole('button', { name: '右リピーターを表示' }).scrollIntoViewIfNeeded()
  await expect(switcher.getByRole('button', { name: '右リピーターを表示' })).toBeVisible()
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  await expect(page.locator('video')).toHaveCount(6)
  await expect(page.locator('.camera-tile:visible')).toHaveCount(1)
  await page.getByRole('button', { name: '右ピラーを表示' }).click()
  await expect(page.getByRole('button', { name: '右ピラーを表示' })).toHaveAttribute('aria-pressed', 'true')
  await page.getByRole('group', { name: '表示するカメラ' }).scrollIntoViewIfNeeded()
  const transitions = await page.locator('.camera-tile, .camera-switcher button, .playback-controls button').evaluateAll((elements) => elements.map((element) => getComputedStyle(element).transitionDuration))
  expect(transitions.every((duration) => duration === '0s')).toBe(true)
  await page.screenshot({ path: `${evidenceRoot}/375-switcher.png` })
  await page.setViewportSize({ width: 1280, height: 1000 })
  await page.getByRole('button', { name: '右ピラーカメラを拡大' }).click()
  await page.screenshot({ path: `${evidenceRoot}/desktop-expanded.png` })
  await context.close()
})
