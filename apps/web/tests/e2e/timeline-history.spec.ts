import { readFile } from 'node:fs/promises'

import { expect, test } from '@playwright/test'

import { captureRuntimeErrors, evidenceRoot, eventRow, fixturePath, hasNoHorizontalScroll, installPicker, selectFolder } from './support/viewer-fixtures'

test('opens dense records from the rail and moves between recorded days with autoplay', async ({ context, page }) => {
  const media = [...await readFile(fixturePath)]
  await installPicker(context, media, 'history')
  const runtimeErrors = captureRuntimeErrors(page)
  await page.goto('/')
  await selectFolder(page)
  await expect(page.getByRole('button', { name: '日付を選ぶ（8月20日 木曜日）' })).toBeVisible()
  await eventRow(page, '2026-08-20 20:35:00').click()

  const cluster = page.getByRole('button', { name: '20:35–20:45・3件を開く' })
  await expect(cluster).toBeVisible()
  const track = await page.locator('.rail__track').boundingBox()
  const pill = await cluster.boundingBox()
  if (track === null || pill === null) throw new Error('rail geometry is missing')
  expect(pill.x).toBeGreaterThanOrEqual(track.x)
  expect(pill.x + pill.width).toBeLessThanOrEqual(track.x + track.width + 0.5)
  await cluster.click()
  await expect(page.getByRole('button', { name: '20:35–20:45・3件を閉じる' })).toHaveAttribute('aria-expanded', 'true')
  await expect(page.getByRole('group', { name: 'イベント一覧' }).getByRole('button')).toHaveCount(3)
  await page.screenshot({ path: `${evidenceRoot}/1280x1000-cluster.png` })
  await eventRow(page, '2026-08-20 20:40:00').click()
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
  await expect(eventRow(page, '2026-08-20 20:40:00')).toHaveAttribute('aria-pressed', 'true')

  await page.getByRole('button', { name: '前の記録日' }).click()
  await expect(page.getByLabel('日付', { exact: true })).toHaveValue('2026-08-19')
  await expect(page.getByRole('button', { name: '日付を選ぶ（8月19日 水曜日）' })).toBeVisible()
  await expect(eventRow(page, '2026-08-19 19:30:00')).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
  await expect(page.getByRole('button', { name: '前の記録日' })).toBeDisabled()

  await page.getByRole('button', { name: '次の記録日' }).click()
  await expect(page.getByLabel('日付', { exact: true })).toHaveValue('2026-08-20')
  await expect(eventRow(page, '2026-08-20 20:45:00')).toHaveAttribute('aria-pressed', 'true')
  expect(runtimeErrors).toEqual([])
})

test('picks a day from the date picker and plays its newest event', async ({ context, page }) => {
  const media = [...await readFile(fixturePath)]
  await installPicker(context, media, 'history')
  await page.goto('/')
  await selectFolder(page)

  await page.getByLabel('日付', { exact: true }).fill('2026-08-19')

  await expect(eventRow(page, '2026-08-19 19:30:00')).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
  await page.getByLabel('日付', { exact: true }).fill('2026-08-10')
  await expect(page.getByText('この日のイベントはありません。')).toBeVisible()
  await expect(page.getByRole('heading', { name: 'イベントを選択してください' })).toBeVisible()
})

test('keeps the day rail usable on a phone without page-wide horizontal scrolling', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  const context = await browser.newContext({ viewport: { width: 375, height: 812 } })
  await installPicker(context, media, 'history')
  const page = await context.newPage()
  await page.goto('/')
  await selectFolder(page)

  await page.getByRole('button', { name: '20:35–20:45・3件を開く' }).click()
  await expect(page.getByRole('group', { name: 'イベント一覧' }).getByRole('button')).toHaveCount(3)
  expect(await hasNoHorizontalScroll(page)).toBe(true)
  await page.getByRole('button', { name: '前の記録日' }).click()
  await expect(page.getByLabel('日付', { exact: true })).toHaveValue('2026-08-19')
  await expect(page.getByRole('button', { name: '2026-08-19 19:30:00 保存済み' })).toBeVisible()
  await page.screenshot({ path: `${evidenceRoot}/375x812-history.png` })
  expect(await hasNoHorizontalScroll(page)).toBe(true)
  await context.close()
})
