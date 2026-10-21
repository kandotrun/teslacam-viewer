import { readFile } from 'node:fs/promises'

import { expect, test } from '@playwright/test'

import { captureRuntimeErrors, fixturePath, installPicker, selectFolder, textSpansSingleLine } from './support/viewer-fixtures'

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
  const eventListPanel = page.getByRole('region', { name: 'イベント', exact: true })
  for (const category of categories) {
    await page.getByRole('button', { name: category.name }).click()
    await expect(eventListPanel.getByText(category.count, { exact: true })).toBeVisible()
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
  await page.getByRole('button', { name: '過去の記録' }).click()
  await expect(page.getByRole('button', { name: '2026-08-20 20:40:00', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await page.getByRole('button', { name: '新しい記録' }).click()
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

test('autoplays each selected event at its local offset while keeping displayed Sentry location metadata local', async ({ context, page }) => {
  const media = [...await readFile(fixturePath)]
  await installPicker(context, media, 'full')
  const requests: Array<{ readonly url: string; readonly body: string }> = []
  page.on('request', (request) => {
    if (/^https?:/u.test(request.url())) requests.push({ url: request.url(), body: request.postData() ?? '' })
  })
  await page.goto('/')
  await selectFolder(page)

  await page.locator('.event-list').getByRole('button', { name: '2026-08-20 20:35:00', exact: true }).click()
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
  await expect.poll(async () => Number(await page.getByLabel('再生位置').inputValue())).toBeGreaterThanOrEqual(15)
  await expect(page.getByText('検知の15秒前から開始', { exact: true })).toBeVisible()
  const detectionReason = page.getByText('sentry_aware_object_detection', { exact: true })
  await expect(detectionReason).not.toBeVisible()
  await page.getByText('技術情報', { exact: true }).click()
  await expect(detectionReason).toBeVisible()
  await expect(page.getByText('2026-08-20T20:35:30', { exact: true })).toBeVisible()
  expect(await textSpansSingleLine(page.locator('.event-card[data-selected="true"]'), 'セントリー')).toBe(true)
  await expect(page.getByText('場所（Teslaの推定値）', { exact: true })).toBeVisible()
  await expect(page.getByText('SYNTHETIC_PRIVATE_CITY SYNTHETIC_PRIVATE_STREET', { exact: true })).toBeVisible()
  await expect(page.getByText('推定座標', { exact: true })).toBeVisible()
  await expect(page.getByText('SYNTHETIC_PRIVATE_LAT, SYNTHETIC_PRIVATE_LON', { exact: true })).toBeVisible()

  await page.getByRole('button', { name: '一時停止' }).click()
  await page.getByRole('button', { name: '新しい記録' }).click()
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
  await expect.poll(async () => Number(await page.getByLabel('再生位置').inputValue())).toBeLessThan(1)

  await page.getByRole('button', { name: '一時停止' }).click()
  await page.getByRole('button', { name: '過去の記録' }).click()
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
  await expect.poll(async () => Number(await page.getByLabel('再生位置').inputValue())).toBeGreaterThanOrEqual(15)

  await page.getByRole('button', { name: '一時停止' }).click()
  await page.getByRole('button', { name: '20:35–20:45・3件を開く' }).click()
  await page.getByRole('button', { name: '2026-08-20 20:45:00 最近の走行', exact: true }).click()
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
  await expect.poll(async () => Number(await page.getByLabel('再生位置').inputValue())).toBeLessThan(1)

  const privateValues = [
    'SYNTHETIC_PRIVATE_CITY',
    'SYNTHETIC_PRIVATE_STREET',
    'SYNTHETIC_PRIVATE_LAT',
    'SYNTHETIC_PRIVATE_LON',
    '2026-08-20_20-35-00',
    'sentry-autoplay-20s.mp4',
    'event.json',
  ]
  const pageOrigin = new URL(page.url()).origin
  expect(requests.every((request) => new URL(request.url).origin === pageOrigin)).toBe(true)
  expect(requests.every((request) => privateValues.every((value) => !`${request.url} ${request.body}`.includes(value)))).toBe(true)
})
