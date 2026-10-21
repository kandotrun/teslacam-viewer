import { readFile } from 'node:fs/promises'

import { expect, test, type Browser, type Page } from '@playwright/test'

import { eventRow, fixturePath, installPicker } from './support/viewer-fixtures'

async function openApp(browser: Browser, width: number, height: number): Promise<Page> {
  const media = [...await readFile(fixturePath)]
  const context = await browser.newContext({ viewport: { width, height } })
  await installPicker(context, media, 'full')
  const page = await context.newPage()
  await page.goto('/')
  return page
}

function activeName(page: Page): Promise<string | null> {
  return page.evaluate(() => {
    const active = document.activeElement
    if (active === null || active === document.body) return null
    return active.getAttribute('aria-label') ?? active.textContent?.trim() ?? active.tagName
  })
}

test('keeps keyboard focus in a meaningful place when the desktop screen changes', async ({ browser }) => {
  const page = await openApp(browser, 1440, 900)

  const select = page.getByRole('button', { name: 'フォルダを選ぶ', exact: true })
  await select.focus()
  await page.keyboard.press('Tab')
  expect(await activeName(page), 'the hidden compatibility input is not a Tab stop').toBeNull()

  await select.focus()
  await page.keyboard.press('Enter')
  await expect(page.getByRole('button', { name: '別のフォルダを選ぶ' })).toBeVisible()
  await expect(page.locator('.event-row:focus')).toHaveCount(1)

  await page.getByRole('button', { name: '検索を開く' }).click()
  await expect(page.getByRole('searchbox', { name: '検索' })).toBeFocused()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('button', { name: '検索を開く' })).toBeFocused()
  await page.context().close()
})

test('moves focus into the viewer on the phone push and back to the opened row on the pop', async ({ browser }) => {
  const page = await openApp(browser, 375, 812)
  await page.getByRole('button', { name: 'フォルダを選ぶ', exact: true }).click()

  const row = eventRow(page, '2026-08-20 20:35:00')
  await row.focus()
  await page.keyboard.press('Enter')
  await expect(page.getByRole('button', { name: '一覧へ' })).toBeFocused()

  await page.keyboard.press('Enter')
  await expect(row).toBeFocused()
  await page.context().close()
})

test('closes the tablet その他の操作 menu with Escape and an outside press', async ({ browser }) => {
  const page = await openApp(browser, 768, 1024)
  await page.getByRole('button', { name: 'フォルダを選ぶ', exact: true }).click()
  await eventRow(page, '2026-08-20 20:35:00').click()

  const more = page.getByRole('button', { name: 'その他の操作' })
  await more.click()
  const menuId = await more.getAttribute('aria-controls')
  expect(menuId).not.toBeNull()
  await expect(page.locator(`[id="${menuId ?? ''}"]`)).toBeVisible()
  await page.getByRole('button', { name: '10秒戻る' }).focus()
  await page.keyboard.press('Escape')
  await expect(page.locator('.transport__menu')).toHaveCount(0)
  await expect(more).toBeFocused()

  await more.click()
  await expect(page.locator('.transport__menu')).toHaveCount(1)
  await page.locator('.stage').click({ position: { x: 5, y: 5 } })
  await expect(page.locator('.transport__menu')).toHaveCount(0)
  await page.context().close()
})
