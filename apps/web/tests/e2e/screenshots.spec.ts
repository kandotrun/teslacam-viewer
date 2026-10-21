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
    await context.close()
  }
})
