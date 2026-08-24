import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'

import { expect, test } from '@playwright/test'

import { captureRuntimeErrors, evidenceRoot, fixturePath, installPicker, selectFolder, textSpansSingleLine, viewports } from './support/viewer-fixtures'

test('shows missing and terminal media states with representative fallback', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  for (const viewport of viewports) {
    const missingContext = await browser.newContext({ viewport: { width: viewport.width, height: viewport.height } })
    await installPicker(missingContext, media, 'missing')
    const missingPage = await missingContext.newPage()
    await missingPage.goto('/')
    await selectFolder(missingPage)
    const partialEvent = missingPage.getByRole('button', { name: /2026-08-20 20:35:00/ })
    await expect(partialEvent).toContainText('5/6 カメラ')
    await expect(partialEvent).toContainText('右リピーターなし')
    await partialEvent.click()
    if (viewport.mode === 'mobile') {
      await missingPage.getByRole('button', { name: '右リピーターを表示' }).click()
    }
    await expect(missingPage.getByText('映像がありません')).toBeVisible()
    await expect(missingPage.getByLabel('再生中のイベント')).toContainText('5/6 カメラ')
    await missingPage.screenshot({ path: `${evidenceRoot}/${viewport.name}-missing.png` })
    expect(await missingPage.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
    await missingContext.close()
  }

  const corruptContext = await browser.newContext({ viewport: { width: 1280, height: 1000 } })
  await installPicker(corruptContext, media, 'corrupt')
  const corruptPage = await corruptContext.newPage()
  await corruptPage.goto('/')
  await selectFolder(corruptPage)
  await corruptPage.getByRole('button', { name: /2026-08-20 20:30:00/ }).click()
  await expect(corruptPage.locator('.camera-tile[data-status="corrupt"], .camera-tile[data-status="unsupported"]')).toHaveCount(6)
  await expect(corruptPage.getByText('代表映像（単独再生）')).toBeVisible()
  expect(await textSpansSingleLine(corruptPage.getByText('6カメラ同期映像ではありません。'), 'ありません。')).toBe(true)
  const terminalGrid = await corruptPage.locator('.camera-grid').boundingBox()
  const representative = await corruptPage.locator('.representative-video').boundingBox()
  const transport = await corruptPage.locator('.playback-controls').boundingBox()
  const viewportHeight = await corruptPage.evaluate(() => window.innerHeight)
  if (terminalGrid === null || representative === null || transport === null) throw new Error('terminal layout geometry is missing')
  const maximumTileBottom = await corruptPage.locator('.camera-tile').evaluateAll((elements) => Math.max(...elements.map((element) => {
    const bounds = element.getBoundingClientRect()
    return bounds.y + bounds.height
  })))
  const internalOverlapCount = await corruptPage.locator('.camera-tile').evaluateAll((elements) => elements.filter((element) => {
    const label = element.querySelector('.camera-tile__label')
    const state = element.querySelector('.camera-tile__state')
    if (label === null || state === null) return false
    const labelBounds = label.getBoundingClientRect()
    const stateBounds = state.getBoundingClientRect()
    return labelBounds.left < stateBounds.right && labelBounds.right > stateBounds.left && labelBounds.top < stateBounds.bottom && labelBounds.bottom > stateBounds.top
  }).length)
  expect(maximumTileBottom).toBeLessThanOrEqual(representative.y)
  expect(internalOverlapCount).toBe(0)
  expect(representative.y + representative.height).toBeLessThanOrEqual(transport.y)
  expect(transport.y + transport.height).toBeLessThanOrEqual(viewportHeight)
  expect(await corruptPage.locator('.playback-controls__rate select').evaluateAll((elements, height) => elements.every((element) => {
    const bounds = element.getBoundingClientRect()
    return bounds.top >= 0 && bounds.bottom <= height
  }), viewportHeight)).toBe(true)
  await corruptPage.screenshot({ path: `${evidenceRoot}/1280-terminal-fallback.png` })
  await corruptContext.close()
})

test('handles wrong folders, permission denial, cancellation and unknown picker failures', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  const scenarios = [
    { variant: 'wrong', expected: 'TeslaCamフォルダを確認できませんでした。' },
    { variant: 'denied', expected: 'フォルダへのアクセスが許可されませんでした。' },
    { variant: 'primitive', expected: 'フォルダの読み取りを完了できませんでした。' },
  ] as const
  for (const scenario of scenarios) {
    const context = await browser.newContext()
    await installPicker(context, media, scenario.variant)
    const page = await context.newPage()
    const runtimeErrors = captureRuntimeErrors(page)
    await page.goto('/')
    await page.getByRole('button', { name: 'TeslaCamフォルダを選ぶ', exact: true }).click()
    await expect(page.getByText(scenario.expected)).toBeVisible()
    expect(runtimeErrors).toEqual([])
    await context.close()
  }
  const cancelledContext = await browser.newContext()
  await installPicker(cancelledContext, media, 'cancelled')
  const cancelledPage = await cancelledContext.newPage()
  await cancelledPage.goto('/')
  await cancelledPage.getByRole('button', { name: 'TeslaCamフォルダを選ぶ', exact: true }).click()
  await expect(cancelledPage.getByRole('heading', { name: 'SSDのTeslaCamフォルダを開く' })).toBeVisible()
  await cancelledContext.close()
})

test('uses the real directory-input fallback with webkitRelativePath', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'showDirectoryPicker', { configurable: true, value: undefined })
  })
  await page.goto('/')
  await expect(page.getByRole('status', { name: 'ブラウザ互換性' })).toContainText('互換モード')
  const chooser = page.waitForEvent('filechooser')
  await page.getByRole('button', { name: 'TeslaCamフォルダを選ぶ', exact: true }).click()
  const fileChooser = await chooser
  await fileChooser.setFiles(fileURLToPath(new URL('./fixtures/TeslaCam', import.meta.url)))
  await expect(page.getByRole('button', { name: '別のフォルダを選ぶ' })).toBeVisible()
  await expect(page.getByRole('button', { name: /^すべて\s*1$/u })).toBeVisible()
})
