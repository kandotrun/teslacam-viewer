import { readFile } from 'node:fs/promises'

import { expect, test, type Page } from '@playwright/test'

import { evidenceRoot, eventRow, fixturePath, hasNoHorizontalScroll, installPicker, selectFolder } from './support/viewer-fixtures'

async function centre(page: Page, selector: string): Promise<{ readonly x: number; readonly y: number }> {
  const bounds = await page.locator(selector).boundingBox()
  if (bounds === null) throw new Error(`${selector} has no layout`)
  return { x: bounds.x + bounds.width / 2, y: bounds.y + bounds.height / 2 }
}

async function openSentry(page: Page): Promise<void> {
  await page.goto('/')
  await selectFolder(page)
  await eventRow(page, '2026-08-20 20:35:00').click()
  await expect(page.locator('.camera-window[data-status="ready"]:visible').first()).toBeVisible()
}

test('places every camera window where the camera sits on the car', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  const context = await browser.newContext({ viewport: { width: 1440, height: 900 } })
  await installPicker(context, media, 'full')
  const page = await context.newPage()
  await openSentry(page)

  const car = await centre(page, '.stage__car')
  const at = (camera: string) => centre(page, `.camera-window[data-camera="${camera}"]`)
  const front = await at('front')
  const back = await at('back')
  const leftPillar = await at('left_pillar')
  const rightPillar = await at('right_pillar')
  const leftRepeater = await at('left_repeater')
  const rightRepeater = await at('right_repeater')
  expect(front.y).toBeLessThan(car.y)
  expect(back.y).toBeGreaterThan(car.y)
  expect(leftPillar.x).toBeLessThan(car.x)
  expect(rightPillar.x).toBeGreaterThan(car.x)
  expect(leftRepeater.x).toBeLessThan(car.x)
  expect(leftRepeater.y).toBeGreaterThan(car.y)
  expect(rightRepeater.x).toBeGreaterThan(car.x)
  expect(rightRepeater.y).toBeGreaterThan(car.y)

  const wedges = page.locator('.stage__wedges polygon')
  await expect(wedges).toHaveCount(6)
  await expect(page.locator('.stage__wedges polygon[data-active="true"]')).toHaveAttribute('data-camera', 'front')
  const before = await page.locator('.stage__wedges polygon[data-camera="front"]').getAttribute('points')
  await page.setViewportSize({ width: 1280, height: 800 })
  await expect.poll(() => page.locator('.stage__wedges polygon[data-camera="front"]').getAttribute('points')).not.toBe(before)
  await page.getByRole('button', { name: '左ピラーカメラを拡大' }).click()
  await page.keyboard.press('Escape')
  await expect(page.locator('.stage__wedges polygon[data-active="true"]')).toHaveAttribute('data-camera', 'left_pillar')
  expect(await hasNoHorizontalScroll(page)).toBe(true)
  await context.close()
})

test('keeps the same six videos playing through 6分割, expansion and back without reloading', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 } })
  await installPicker(context, media, 'full')
  const page = await context.newPage()
  await openSentry(page)
  await expect(page.locator('.camera-window[data-status="ready"]')).toHaveCount(6)

  const videos = page.locator('.camera-window video')
  await videos.evaluateAll((elements) => elements.forEach((element, index) => {
    element.dataset['node'] = String(index)
    element.dataset['reloads'] = '0'
    element.addEventListener('emptied', () => {
      element.dataset['reloads'] = String(Number(element.dataset['reloads']) + 1)
    })
  }))

  await page.getByRole('radio', { name: '6分割' }).click()
  await expect(page.locator('.stage')).toHaveAttribute('data-mode', 'grid')
  const tiles = await page.locator('.camera-window').evaluateAll((elements) => elements.map((element) => {
    const bounds = element.getBoundingClientRect()
    return { left: Math.round(bounds.left), top: Math.round(bounds.top) }
  }))
  expect(new Set(tiles.map((tile) => tile.left)).size).toBe(3)
  expect(new Set(tiles.map((tile) => tile.top)).size).toBe(2)
  await page.screenshot({ path: `${evidenceRoot}/1280x800-grid.png` })

  await page.getByRole('button', { name: 'フロントカメラを拡大' }).click()
  await expect(page.locator('.stage')).toHaveAttribute('data-mode', 'expanded')
  await expect(page.getByRole('group', { name: '表示するカメラ' })).toBeVisible()
  await page.getByRole('button', { name: 'リアを表示' }).click()
  await expect(page.getByRole('button', { name: 'リアカメラをグリッドへ戻す' })).toBeVisible()
  await page.screenshot({ path: `${evidenceRoot}/1280x800-expanded.png` })
  await page.keyboard.press('Escape')
  await expect(page.locator('.stage')).toHaveAttribute('data-mode', 'grid')
  await page.getByRole('radio', { name: '車両配置' }).click()

  expect(await videos.evaluateAll((elements) => elements.map((element) => element.dataset['node']))).toEqual(['0', '1', '2', '3', '4', '5'])
  expect(await videos.evaluateAll((elements) => elements.map((element) => element.dataset['reloads']))).toEqual(['0', '0', '0', '0', '0', '0'])
  await expect(page.locator('.camera-window[data-status="ready"]')).toHaveCount(6)
  await context.close()
})

test('hides the library on a tablet to give the stage the whole floor', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  const context = await browser.newContext({ viewport: { width: 768, height: 1024 } })
  await installPicker(context, media, 'full')
  const page = await context.newPage()
  await openSentry(page)
  const narrow = await page.locator('.stage').boundingBox()

  await page.getByRole('button', { name: 'ライブラリを隠す' }).click()
  await expect(page.getByRole('complementary', { name: 'ライブラリ' })).toBeHidden()
  const wide = await page.locator('.stage').boundingBox()
  expect(wide?.width ?? 0).toBeGreaterThan((narrow?.width ?? 0) + 300)
  await page.getByRole('button', { name: 'その他の操作' }).click()
  await page.getByRole('button', { name: '10秒進む' }).click()
  await page.screenshot({ path: `${evidenceRoot}/768x1024-library-hidden.png` })
  await page.getByRole('button', { name: 'ライブラリを表示' }).click()
  await expect(page.getByRole('complementary', { name: 'ライブラリ' })).toBeVisible()
  expect(await hasNoHorizontalScroll(page)).toBe(true)
  await context.close()
})

test('pushes the viewer on a phone and switches cameras on the mini car', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  const context = await browser.newContext({ viewport: { width: 375, height: 812 } })
  await installPicker(context, media, 'full')
  const page = await context.newPage()
  await openSentry(page)

  await expect(page.locator('video')).toHaveCount(6)
  await expect(page.locator('.camera-window:visible')).toHaveCount(1)
  const switcher = page.getByRole('group', { name: '表示するカメラ' })
  await expect(switcher.getByRole('button')).toHaveCount(6)
  await switcher.getByRole('button', { name: '右ピラーを表示' }).click()
  await expect(page.locator('.camera-window:visible')).toHaveAttribute('data-camera', 'right_pillar')
  await expect(switcher.getByRole('button', { name: '右ピラーを表示' })).toHaveAttribute('aria-pressed', 'true')
  const transport = await page.locator('.transport').boundingBox()
  expect(transport?.height ?? 999).toBeLessThanOrEqual(140)
  await page.screenshot({ path: `${evidenceRoot}/375x812-switcher.png` })

  await page.getByRole('button', { name: '一覧へ' }).click()
  await expect(page.getByRole('complementary', { name: 'ライブラリ' })).toBeVisible()
  await expect(page.locator('video')).toHaveCount(0)
  await expect(eventRow(page, '2026-08-20 20:35:00')).toHaveAttribute('aria-pressed', 'true')
  expect(await hasNoHorizontalScroll(page)).toBe(true)
  await context.close()
})
