import { readFile } from 'node:fs/promises'

import { expect, test } from '@playwright/test'

import { evidenceRoot, fixturePath, installPicker, selectFolder } from './support/viewer-fixtures'

const screens = [
  { name: '1440x900', width: 1440, height: 900 },
  { name: '1280x800', width: 1280, height: 800 },
  { name: '768x1024', width: 768, height: 1024 },
  { name: '375x812', width: 375, height: 812 },
] as const

test('captures the welcome and viewer screens for visual review', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  for (const screen of screens) {
    const context = await browser.newContext({ viewport: { width: screen.width, height: screen.height } })
    await installPicker(context, media, 'full')
    const page = await context.newPage()
    await page.goto('/')
    await expect(page.getByRole('heading', { name: 'TeslaCamの記録を見る' })).toBeVisible()
    await page.evaluate(() => document.fonts.ready)
    await page.screenshot({ path: `${evidenceRoot}/${screen.name}-welcome.png` })
    await selectFolder(page)
    await page.getByRole('button', { name: '2026-08-20 20:35:00', exact: true }).click()
    await expect(page.locator('.camera-window[data-status="ready"]:visible').first()).toBeVisible()
    await page.getByRole('button', { name: '一時停止' }).click()
    await page.waitForTimeout(400)
    await page.screenshot({ path: `${evidenceRoot}/${screen.name}-viewer.png` })
    await page.locator('.event-pill').click()
    await expect(page.getByRole('dialog', { name: 'イベント詳細' })).toBeVisible()
    await page.screenshot({ path: `${evidenceRoot}/${screen.name}-details.png` })
    await context.close()
  }
})

test('captures the scanning and error states of the library panel', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  const scanning = await browser.newContext({ viewport: { width: 1440, height: 900 } })
  await installPicker(scanning, media, 'full')
  const scanningPage = await scanning.newPage()
  await scanningPage.goto('/')
  await scanningPage.evaluate(() => {
    const original = window.showDirectoryPicker
    if (original === undefined) return
    Object.defineProperty(window, 'showDirectoryPicker', {
      configurable: true,
      value: async () => {
        const root = await original()
        return {
          kind: 'directory',
          name: root.name,
          async *values() {
            for await (const entry of root.values()) {
              yield entry
              await new Promise((resolve) => setTimeout(resolve, 400))
            }
          },
        }
      },
    })
  })
  await scanningPage.getByRole('button', { name: 'フォルダを選ぶ', exact: true }).click()
  await expect(scanningPage.getByText('件を確認しました')).toBeVisible()
  await scanningPage.screenshot({ path: `${evidenceRoot}/1440x900-scanning.png` })
  await scanning.close()

  const failing = await browser.newContext({ viewport: { width: 1440, height: 900 } })
  await installPicker(failing, media, 'wrong')
  const failingPage = await failing.newPage()
  await failingPage.goto('/')
  await failingPage.getByRole('button', { name: 'フォルダを選ぶ', exact: true }).click()
  await expect(failingPage.getByRole('alert')).toBeVisible()
  await failingPage.screenshot({ path: `${evidenceRoot}/1440x900-error.png` })
  await failing.close()
})

