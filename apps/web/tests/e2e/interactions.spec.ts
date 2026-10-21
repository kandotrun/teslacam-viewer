import { readFile } from 'node:fs/promises'

import { expect, test } from '@playwright/test'

import {
  captureRuntimeErrors,
  eventRow,
  fixturePath,
  installPicker,
  playbackPosition,
  sameOriginRequests,
  selectFolder,
} from './support/viewer-fixtures'

const privateValues = [
  'SYNTHETIC_PRIVATE_CITY',
  'SYNTHETIC_PRIVATE_STREET',
  'SYNTHETIC_PRIVATE_LAT',
  'SYNTHETIC_PRIVATE_LON',
  '2026-08-20_20',
  'sentry-autoplay-20s.mp4',
  'event.json',
  'synthetic-',
  'ftyp',
]

test('drives categories, autoplay, the transport and keyboard shortcuts without an HTTP leak', async ({ context, page }) => {
  const media = [...await readFile(fixturePath)]
  await installPicker(context, media, 'full')
  const runtimeErrors = captureRuntimeErrors(page)
  const network = sameOriginRequests(page)
  await page.goto('/')
  await selectFolder(page)

  const rows = page.getByRole('group', { name: 'イベント一覧' }).getByRole('button')
  for (const [name, count] of [['すべて 3件', 3], ['最近の走行 1件', 1], ['保存済み 1件', 1], ['セントリー 1件', 1]] as const) {
    await page.getByRole('radio', { name }).click()
    await expect(page.getByRole('radio', { name })).toBeChecked()
    await expect(rows).toHaveCount(count)
  }
  await page.getByRole('radio', { name: 'すべて 3件' }).click()

  await eventRow(page, '2026-08-20 20:45:00').click()
  await expect(page.locator('.camera-window[data-status="ready"]')).toHaveCount(6)
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
  await expect.poll(() => playbackPosition(page)).toBeGreaterThan(0)

  await page.evaluate(() => {
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur()
  })
  await page.keyboard.press('Space')
  await expect(page.getByRole('button', { name: '再生' })).toBeVisible()
  await page.keyboard.press('ArrowRight')
  expect(await playbackPosition(page)).toBeGreaterThan(1)
  await page.keyboard.press('Digit6')
  await expect(page.getByRole('button', { name: '右リピーターカメラを縮小' })).toBeVisible()
  await expect(page.locator('.stage')).toHaveAttribute('data-mode', 'expanded')
  await page.keyboard.press('Escape')
  await expect(page.getByRole('button', { name: '右リピーターカメラを拡大' })).toBeVisible()
  await page.keyboard.press('BracketLeft')
  await expect(eventRow(page, '2026-08-20 20:40:00')).toHaveAttribute('aria-pressed', 'true')
  await page.keyboard.press('BracketRight')
  await expect(eventRow(page, '2026-08-20 20:45:00')).toHaveAttribute('aria-pressed', 'true')

  await page.getByRole('button', { name: '過去の記録' }).click()
  await expect(eventRow(page, '2026-08-20 20:40:00')).toHaveAttribute('aria-pressed', 'true')
  await page.getByRole('button', { name: '10秒戻る' }).click()
  await page.getByRole('combobox', { name: '再生速度' }).selectOption('1.5')
  await expect(page.getByRole('combobox', { name: '再生速度' })).toHaveValue('1.5')
  await page.getByRole('button', { name: '新しい記録' }).click()
  await expect(page.getByRole('combobox', { name: '再生速度' })).toHaveValue('1')
  await expect(page.getByRole('button', { name: '新しい記録' })).toBeDisabled()

  const pageOrigin = new URL(page.url()).origin
  expect(network.requests.every((request) => request.startsWith(pageOrigin))).toBe(true)
  expect(network.requests.filter((request) => privateValues.some((value) => request.includes(value)))).toEqual([])
  expect(runtimeErrors).toEqual([])
})

test('starts a Sentry event 15 s before the detection and keeps its location on the device', async ({ context, page }) => {
  const media = [...await readFile(fixturePath)]
  await installPicker(context, media, 'full')
  const network = sameOriginRequests(page)
  await page.goto('/')
  await selectFolder(page)

  await eventRow(page, '2026-08-20 20:35:00').click()
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
  await expect.poll(() => playbackPosition(page)).toBeGreaterThanOrEqual(15)
  await expect(page.getByText('検知の15秒前', { exact: true })).toBeVisible()
  await expect(page.getByRole('img', { name: '検知位置' })).toBeVisible()
  await expect(eventRow(page, '2026-08-20 20:35:00')).toContainText('20:35:30に検知')
  await expect(page.locator('.event-pill')).toContainText('物体の接近を20:35:30に検知')

  await page.locator('.event-pill').click()
  const details = page.getByRole('dialog', { name: 'イベント詳細' })
  await expect(details.getByText('SYNTHETIC_PRIVATE_CITY SYNTHETIC_PRIVATE_STREET', { exact: true })).toBeVisible()
  await expect(details.getByText('SYNTHETIC_PRIVATE_LAT, SYNTHETIC_PRIVATE_LON', { exact: true })).toBeVisible()
  await expect(details.getByText('20:35:15（検知の15秒前）', { exact: true })).toBeVisible()
  const reason = details.getByText('sentry_aware_object_detection', { exact: true })
  await expect(reason).toBeHidden()
  await details.getByText('技術情報', { exact: true }).click()
  await expect(reason).toBeVisible()
  await expect(details.getByRole('link')).toHaveCount(0)
  await page.keyboard.press('Escape')
  await expect(details).toBeHidden()

  await page.getByRole('button', { name: '新しい記録' }).click()
  await expect(eventRow(page, '2026-08-20 20:40:00')).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
  await expect.poll(() => playbackPosition(page)).toBeLessThan(1)
  await expect(page.getByText('検知の15秒前', { exact: true })).toHaveCount(0)
  await expect(page.getByRole('img', { name: '検知位置' })).toHaveCount(0)

  await page.getByRole('button', { name: '過去の記録' }).click()
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
  await expect.poll(() => playbackPosition(page)).toBeGreaterThanOrEqual(15)

  await page.getByRole('button', { name: '20:35–20:45・3件を開く' }).click()
  await expect(page.getByRole('group', { name: '20:35–20:45・3件' })).toBeVisible()
  await eventRow(page, '2026-08-20 20:45:00').click()
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
  await expect.poll(() => playbackPosition(page)).toBeLessThan(1)

  const pageOrigin = new URL(page.url()).origin
  expect(network.requests.every((request) => request.startsWith(pageOrigin))).toBe(true)
  expect(network.requests.filter((request) => privateValues.some((value) => request.includes(value)))).toEqual([])
})

test('replays the last segment when play is pressed at the end', async ({ context, page }) => {
  const media = [...await readFile(fixturePath)]
  await installPicker(context, media, 'full')
  await page.goto('/')
  await selectFolder(page)
  await eventRow(page, '2026-08-20 20:45:00').click()
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()

  await expect(page.getByRole('button', { name: '再生' })).toBeVisible({ timeout: 10_000 })
  const ended = await playbackPosition(page)
  await page.getByRole('button', { name: '再生' }).click()
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
  await expect.poll(() => playbackPosition(page)).not.toBe(ended)
})
