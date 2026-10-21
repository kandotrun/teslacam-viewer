import { readFile } from 'node:fs/promises'

import { expect, test, type Browser, type Locator, type Page } from '@playwright/test'

import { evidenceRoot, eventRow, fixturePath, installPicker, selectFolder } from './support/viewer-fixtures'

type Box = { readonly x: number; readonly y: number; readonly width: number; readonly height: number }

async function openLibrary(browser: Browser, width: number, height: number): Promise<Page> {
  const media = [...await readFile(fixturePath)]
  const context = await browser.newContext({ viewport: { width, height } })
  await installPicker(context, media, 'missing')
  const page = await context.newPage()
  await page.goto('/')
  await selectFolder(page)
  await page.evaluate(() => document.fonts.ready)
  return page
}

async function boxOf(locator: Locator): Promise<Box> {
  const bounds = await locator.boundingBox()
  if (bounds === null) throw new Error('element has no layout')
  return bounds
}

/** Windows of the strip below an expanded camera or event.mp4, with whether their name pill is cut. */
function stripWindows(page: Page): Promise<readonly { readonly width: number; readonly height: number; readonly labelClipped: boolean }[]> {
  return page.locator('.camera-slot[data-area^="s"] .camera-window').evaluateAll((windows) => windows.map((window) => {
    const bounds = window.getBoundingClientRect()
    const label = window.querySelector('.camera-window__label')
    return {
      width: bounds.width,
      height: bounds.height,
      labelClipped: label === null || label.scrollWidth > label.clientWidth + 0.5,
    }
  }))
}

function overflows(locator: Locator): Promise<boolean> {
  return locator.evaluate((element) => element.scrollWidth > element.clientWidth + 0.5)
}

async function settle(page: Page): Promise<void> {
  await page.waitForTimeout(450)
}

for (const screen of [{ name: '768x1024', width: 768, height: 1024 }, { name: '375x812', width: 375, height: 812 }] as const) {
  test(`keeps strips, the event pill and the dialog legible at ${screen.name}`, async ({ browser }) => {
    const page = await openLibrary(browser, screen.width, screen.height)
    const phone = screen.width <= 740

    if (!phone) {
      const placeholder = page.locator('.placeholder-pill')
      await expect(placeholder).toBeVisible()
      await expect(placeholder).toHaveCSS('padding-left', '22px')
      expect(await overflows(placeholder)).toBe(false)
      await page.screenshot({ path: `${evidenceRoot}/${screen.name}-placeholder.png` })
    }

    // Sentry event with a missing right repeater: the fault reads as icon + word without the name pill on top.
    await eventRow(page, '2026-08-20 20:35:00').click()
    await page.getByRole('button', { name: '一時停止' }).click()
    if (phone) {
      await expect(page.getByRole('button', { name: '一覧へ' })).toHaveCSS('padding-right', '14px')
    } else {
      for (const view of ['車両配置', '6分割'] as const) {
        await page.getByRole('radio', { name: view }).click()
        await settle(page)
        const missing = page.locator('.camera-window[data-status="missing"]')
        const window = await boxOf(missing)
        const label = await boxOf(missing.locator('.camera-window__label'))
        const icon = await boxOf(missing.locator('.camera-window__state svg'))
        expect(icon.y, `${view}: icon inside the window`).toBeGreaterThanOrEqual(window.y)
        const text = missing.locator('.camera-window__state-text')
        const textBox = await boxOf(text)
        if (textBox.width > 1) {
          expect(textBox.y + textBox.height, `${view}: fault text clear of the name pill`).toBeLessThanOrEqual(label.y + 0.5)
        }
        await page.screenshot({ path: `${evidenceRoot}/${screen.name}-${view === '6分割' ? 'grid' : 'spatial'}-missing.png` })
      }
      await page.getByRole('radio', { name: '車両配置' }).click()
      await page.getByRole('button', { name: 'フロントカメラを拡大' }).click()
      await settle(page)
      const expanded = await stripWindows(page)
      expect(expanded).toHaveLength(5)
      for (const window of expanded) {
        expect(Math.min(window.width, window.height)).toBeGreaterThanOrEqual(44)
        expect(window.labelClipped).toBe(false)
      }
      await page.screenshot({ path: `${evidenceRoot}/${screen.name}-expanded.png` })
      await page.keyboard.press('Escape')
    }

    // Saved event with event.mp4: the actions pill offers 代表映像 too.
    if (phone) await page.getByRole('button', { name: '一覧へ' }).click()
    await eventRow(page, '2026-08-20 20:40:00').click()
    await page.getByRole('button', { name: '一時停止' }).click()
    const pill = page.locator('.event-pill')
    expect(await overflows(pill), 'the event pill shows its start time in full').toBe(false)
    await page.getByRole('radio', { name: '代表映像' }).click()
    await settle(page)
    expect(await overflows(pill)).toBe(false)
    const representative = await stripWindows(page)
    expect(representative).toHaveLength(6)
    for (const window of representative) {
      expect(Math.min(window.width, window.height)).toBeGreaterThanOrEqual(44)
      expect(window.labelClipped).toBe(false)
    }
    await page.screenshot({ path: `${evidenceRoot}/${screen.name}-representative.png` })

    await page.getByRole('radio', { name: '車両配置' }).click()
    await page.getByRole('button', { name: '合成動画を書き出す' }).click()
    const dialog = page.getByRole('dialog', { name: '合成動画を書き出す' })
    await expect(dialog).toBeVisible()
    await settle(page)
    expect(await overflows(dialog), 'the export dialog has no horizontal overflow').toBe(false)
    await page.screenshot({ path: `${evidenceRoot}/${screen.name}-export.png` })
    await page.context().close()
  })
}

for (const screen of [{ name: '1440x900', width: 1440, height: 900 }, { name: '768x1024', width: 768, height: 1024 }] as const) {
  test(`packs the 6分割 rows and centres the export dialog on the stage at ${screen.name}`, async ({ browser }) => {
    const page = await openLibrary(browser, screen.width, screen.height)
    await eventRow(page, '2026-08-20 20:45:00').click()
    await page.getByRole('button', { name: '一時停止' }).click()
    await page.getByRole('radio', { name: '6分割' }).click()
    await settle(page)
    const windows = await page.locator('.stage[data-mode="grid"] .camera-window').evaluateAll((elements) => elements.map((element) => {
      const bounds = element.getBoundingClientRect()
      return { x: bounds.x, y: bounds.y, right: bounds.right, bottom: bounds.bottom }
    }))
    const [first, second, , fourth] = windows
    if (first === undefined || second === undefined || fourth === undefined) throw new Error('grid windows missing')
    const columnGap = second.x - first.right
    const rowGap = fourth.y - first.bottom
    expect(rowGap, `row gap ${rowGap} vs column gap ${columnGap}`).toBeLessThanOrEqual(columnGap + 8)
    await page.screenshot({ path: `${evidenceRoot}/${screen.name}-grid-packed.png` })

    if (screen.width > 1100) {
      await page.getByRole('button', { name: '合成動画を書き出す' }).click()
      const dialog = page.getByRole('dialog', { name: '合成動画を書き出す' })
      await expect(dialog).toBeVisible()
      await settle(page)
      const dialogBox = await boxOf(dialog)
      const stage = await boxOf(page.locator('.stage'))
      expect(Math.abs((dialogBox.x + dialogBox.width / 2) - (stage.x + stage.width / 2))).toBeLessThanOrEqual(2)
      const cancel = await boxOf(dialog.getByRole('button', { name: 'キャンセル' }))
      const start = await boxOf(dialog.getByRole('button', { name: '書き出しを開始' }))
      expect(Math.abs((cancel.y + cancel.height / 2) - (start.y + start.height / 2))).toBeLessThanOrEqual(0.5)
      await page.screenshot({ path: `${evidenceRoot}/${screen.name}-export-centred.png` })
    }
    await page.context().close()
  })
}

test('ends long place names with an ellipsis and keeps the cluster pill on the rail at 1440x900', async ({ browser }) => {
  const page = await openLibrary(browser, 1440, 900)
  await eventRow(page, '2026-08-20 20:35:00').click()
  await page.getByRole('button', { name: '一時停止' }).click()

  const place = page.locator('.event-pill__where-text')
  await expect(place).toHaveText('SYNTHETIC_PRIVATE_CITY SYNTHETIC_PRIVATE_STREET')
  await expect(place).toHaveCSS('text-overflow', 'ellipsis')
  await expect(place).toHaveCSS('overflow-x', 'hidden')
  expect(await place.evaluate((element) => getComputedStyle(element).display)).not.toMatch(/flex/u)

  const cluster = page.locator('.rail__cluster').first()
  const clusterBox = await boxOf(cluster)
  const pillBox = await boxOf(cluster.locator('.rail__cluster-pill'))
  const track = page.locator('.rail__track')
  const trackBox = await boxOf(track)
  expect(pillBox.width).toBeLessThanOrEqual(clusterBox.width + 0.5)
  expect(pillBox.x + pillBox.width).toBeLessThanOrEqual(trackBox.x + trackBox.width + 0.5)
  expect(await overflows(track)).toBe(false)
  await page.screenshot({ path: `${evidenceRoot}/1440x900-pill-rail.png` })

  await page.locator('.event-pill').click()
  await expect(page.locator('.details__row dd').first()).toHaveCSS('word-break', 'auto-phrase')
  await page.context().close()
})
