import { readFile } from 'node:fs/promises'

import { expect, test } from '@playwright/test'

import { captureRuntimeErrors, fixturePath, installPicker, selectFolder } from './support/viewer-fixtures'

test('drives categories, playback, camera and event keyboard controls without an HTTP leak', async ({ context, page }) => {
  const media = [...await readFile(fixturePath)]
  await installPicker(context, media, 'full')
  const runtimeErrors = captureRuntimeErrors(page)
  const requests: Array<{ readonly url: string; readonly body: string }> = []
  page.on('request', (request) => {
    if (/^https?:/u.test(request.url())) {
      requests.push({ url: request.url(), body: request.postData() ?? '' })
    }
  })
  await page.goto('/')
  await selectFolder(page)
  const categories = [
    { name: /^すべて\s*3$/u, count: '3件' },
    { name: /^最近の走行\s*1$/u, count: '1件' },
    { name: /^保存済み\s*1$/u, count: '1件' },
    { name: /^セントリー\s*1$/u, count: '1件' },
  ] as const
  for (const category of categories) {
    await page.getByRole('button', { name: category.name }).click()
    await expect(page.getByText(category.count, { exact: true })).toBeVisible()
  }
  await page.getByRole('button', { name: /^すべて\s*3$/u }).click()
  await page.getByRole('button', { name: /2026-08-20 20:45:00/ }).click()
  await expect(page.locator('video')).toHaveCount(6)
  await expect(page.locator('.camera-tile[data-status="ready"]')).toHaveCount(6)
  await page.getByRole('button', { name: '再生' }).click()
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
  await expect.poll(async () => Number(await page.getByLabel('再生位置').inputValue())).toBeGreaterThan(0)
  await page.evaluate(() => {
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur()
  })
  await page.keyboard.press('Space')
  await expect(page.getByRole('button', { name: '再生' })).toBeVisible()
  await page.keyboard.press('ArrowRight')
  expect(Number(await page.getByLabel('再生位置').inputValue())).toBeGreaterThan(1)
  await page.keyboard.press('Digit6')
  await expect(page.getByRole('button', { name: '右リピーターカメラをグリッドへ戻す' })).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('button', { name: '右リピーターカメラを拡大' })).toBeVisible()
  await page.getByRole('button', { name: '次のイベント' }).click()
  await expect(page.getByRole('button', { name: '2026-08-20 20:40:00', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await page.getByRole('button', { name: '前のイベント' }).click()
  const visibleControls = page.locator('button:visible')
  const sizes = await visibleControls.evaluateAll((elements) => elements.map((element) => {
    const bounds = element.getBoundingClientRect()
    return { width: bounds.width, height: bounds.height }
  }))
  expect(sizes.every((size) => size.width >= 44 && size.height >= 44)).toBe(true)
  const pageOrigin = new URL(page.url()).origin
  expect(requests.every((request) => new URL(request.url).origin === pageOrigin)).toBe(true)
  expect(requests.some((request) => {
    const requestText = `${request.url} ${request.body}`
    return requestText.includes('2026-08-20_20') || requestText.includes('synthetic-') || requestText.includes('ftyp')
  })).toBe(false)
  expect(runtimeErrors).toEqual([])
})
