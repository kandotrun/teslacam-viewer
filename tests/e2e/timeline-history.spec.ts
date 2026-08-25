import { readFile } from 'node:fs/promises'

import { expect, test } from '@playwright/test'

import { captureRuntimeErrors, evidenceRoot, fixturePath, installPicker, selectFolder } from './support/viewer-fixtures'

test('opens dense records and moves between recorded days with autoplay', async ({ context, page }) => {
  const media = [...await readFile(fixturePath)]
  await installPicker(context, media, 'history')
  const runtimeErrors = captureRuntimeErrors(page)
  await page.goto('/')
  await selectFolder(page)
  await page.locator('.event-list').getByRole('button', { name: '2026-08-20 20:35:00', exact: true }).click()

  const cluster = page.getByRole('button', { name: '20:35–20:45・3件を開く' })
  await expect(cluster).toBeVisible()
  await cluster.click()
  await page.screenshot({ path: `${evidenceRoot}/timeline-cluster-1280.png` })
  await page.getByRole('button', { name: '2026-08-20 20:40:00 保存済み' }).click()
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
  await expect(page.locator('.event-list').getByRole('button', { name: '2026-08-20 20:40:00', exact: true })).toHaveAttribute('aria-pressed', 'true')

  await page.getByRole('button', { name: '前の記録日 2026-08-19' }).click()
  await expect(page.getByLabel('日付')).toHaveValue('2026-08-19')
  await expect(page.locator('.event-list').getByRole('button', { name: '2026-08-19 19:30:00', exact: true })).toHaveAttribute('aria-pressed', 'true')
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()

  await page.getByRole('button', { name: '次の記録日 2026-08-20' }).click()
  await expect(page.getByLabel('日付')).toHaveValue('2026-08-20')
  await expect(page.locator('.event-list').getByRole('button', { name: '2026-08-20 20:45:00', exact: true })).toHaveAttribute('aria-pressed', 'true')
  expect(runtimeErrors).toEqual([])
})

test('offers a vertical history on mobile without page-wide horizontal scrolling', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  const context = await browser.newContext({ viewport: { width: 375, height: 844 } })
  await installPicker(context, media, 'history')
  const page = await context.newPage()
  await page.goto('/')
  await selectFolder(page)
  await page.locator('.event-list').getByRole('button', { name: '2026-08-20 20:35:00', exact: true }).click()

  await page.getByRole('button', { name: '履歴を開く（3件）' }).click()
  await expect(page.getByRole('button', { name: '2026-08-20 20:35:00 セントリー' })).toBeVisible()
  await expect(page.getByRole('button', { name: '2026-08-20 20:45:00 最近の走行' })).toBeVisible()
  await page.screenshot({ path: `${evidenceRoot}/timeline-history-375.png`, fullPage: true })
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)

  await page.getByRole('button', { name: '前の記録日 2026-08-19' }).click()
  await expect(page.getByLabel('日付')).toHaveValue('2026-08-19')
  await expect(page.getByRole('button', { name: '履歴を開く（1件）' })).toBeVisible()
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  await context.close()
})

test('switches timeline presentation when the workspace is resized', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  const context = await browser.newContext({ viewport: { width: 1280, height: 1000 } })
  await installPicker(context, media, 'full')
  const page = await context.newPage()
  await page.goto('/')
  await selectFolder(page)
  await page.locator('.event-list').getByRole('button', { name: '2026-08-20 20:35:00', exact: true }).click()
  const cluster = page.getByRole('button', { name: '20:35–20:45・3件を開く' })
  await expect(cluster).toBeVisible()
  await cluster.click()
  await expect(page.locator('.day-timeline')).toHaveAttribute('data-cluster-expanded', 'true')

  await page.setViewportSize({ width: 375, height: 844 })
  await expect(page.getByRole('button', { name: '履歴を開く（3件）' })).toBeVisible()
  await expect(page.locator('.day-timeline')).toHaveAttribute('data-cluster-expanded', 'false')
  await page.setViewportSize({ width: 768, height: 1024 })
  await expect(page.getByRole('button', { name: '2026-08-20 20:35:00 セントリー' })).toBeVisible()
  await expect(page.getByRole('button', { name: /履歴を開く/u })).toHaveCount(0)
  await expect(page.locator('.day-timeline')).toHaveAttribute('data-cluster-expanded', 'false')
  await context.close()
})
