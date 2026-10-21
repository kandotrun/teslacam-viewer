import { readFile } from 'node:fs/promises'

import { expect, test, type Page } from '@playwright/test'

import {
  captureRuntimeErrors,
  contrastRatio,
  eventRow,
  fixturePath,
  hasNoHorizontalScroll,
  installPicker,
  relativeLuminance,
  textSpansSingleLine,
  viewports,
} from './support/viewer-fixtures'

type Box = { readonly x: number; readonly y: number; readonly width: number; readonly height: number }

function inside(box: Box, width: number, height: number): boolean {
  return box.x >= 0 && box.y >= 0 && box.x + box.width <= width + 0.5 && box.y + box.height <= height + 0.5
}

function overlaps(first: Box, second: Box): boolean {
  return first.x < second.x + second.width && second.x < first.x + first.width && first.y < second.y + second.height && second.y < first.y + first.height
}

async function box(page: Page, selector: string): Promise<Box> {
  const bounds = await page.locator(selector).first().boundingBox()
  if (bounds === null) throw new Error(`${selector} has no layout`)
  return bounds
}

async function undersizedButtons(page: Page, minimum: number): Promise<readonly string[]> {
  return page.locator('button:visible').evaluateAll((buttons, size) => buttons.flatMap((button) => {
    const bounds = button.getBoundingClientRect()
    return bounds.width + 0.5 < size || bounds.height + 0.5 < size
      ? [`${button.getAttribute('aria-label') ?? button.textContent ?? ''} ${Math.round(bounds.width)}x${Math.round(bounds.height)}`]
      : []
  }), minimum)
}

for (const viewport of viewports) {
  test(`lays out the garage floor at ${viewport.name}`, async ({ browser }) => {
    const media = [...await readFile(fixturePath)]
    const context = await browser.newContext({ viewport: { width: viewport.width, height: viewport.height } })
    await installPicker(context, media, 'full')
    const page = await context.newPage()
    const runtimeErrors = captureRuntimeErrors(page)
    await page.goto('/')

    const heading = page.getByRole('heading', { name: 'TeslaCamの記録を見る' })
    await expect(heading).toBeVisible()
    expect(await textSpansSingleLine(heading, '記録を見る')).toBe(true)
    const welcomeColors = await page.locator('body').evaluate((element) => {
      const styles = getComputedStyle(element)
      return { background: styles.backgroundColor, text: styles.color }
    })
    expect(relativeLuminance(welcomeColors.background)).toBeGreaterThan(0.8)
    expect(contrastRatio(welcomeColors.text, welcomeColors.background)).toBeGreaterThanOrEqual(4.5)
    if (viewport.mode === 'phone') {
      await expect(page.getByRole('main', { name: '6台のカメラ' })).toHaveCount(0)
    } else {
      await expect(page.locator('.stage[data-mode="empty"] .camera-window')).toHaveCount(6)
      await expect(page.locator('.stage__wedges polygon')).toHaveCount(6)
    }
    expect(await hasNoHorizontalScroll(page)).toBe(true)

    await page.getByRole('button', { name: 'フォルダを選ぶ', exact: true }).click()
    await expect(page.getByText('件を確認しました').first()).toBeVisible()
    await expect(page.getByRole('button', { name: '別のフォルダを選ぶ' })).toBeVisible()
    await expect(page.getByText('TeslaCam', { exact: true })).toHaveCount(1)
    await expect(page.locator('img.event-thumbnail')).toHaveCount(0)
    await expect(page.getByText('対応外のファイル 253 件を除外しました。')).toBeVisible()

    await eventRow(page, '2026-08-20 20:35:00').click()
    await expect(page.locator('video')).toHaveCount(6)
    await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
    if (viewport.mode === 'phone') {
      await expect(page.getByRole('button', { name: '一覧へ' })).toBeVisible()
      await expect(page.getByRole('complementary', { name: 'ライブラリ' })).toBeHidden()
      await expect(page.locator('.camera-window:visible')).toHaveCount(1)
      await expect(page.locator('.camera-window[data-status="ready"]:visible')).toHaveCount(1)
      const transport = await box(page, '.transport')
      expect(transport.height).toBeLessThanOrEqual(140)
      expect(transport.y + transport.height).toBeLessThanOrEqual(viewport.height)
      const camera = await box(page, '.camera-window:visible')
      expect(camera.y + camera.height).toBeLessThanOrEqual(transport.y)
    } else {
      await expect(page.locator('.camera-window[data-status="ready"]')).toHaveCount(6)
      const library = await box(page, '.library')
      const stage = await box(page, '.stage')
      const eventPill = await box(page, '.event-pill')
      const actions = await box(page, '.actions-pill')
      const transport = await box(page, '.transport')
      for (const region of [library, stage, eventPill, actions, transport]) expect(inside(region, viewport.width, viewport.height)).toBe(true)
      expect(overlaps(library, stage)).toBe(false)
      expect(overlaps(eventPill, actions)).toBe(false)
      expect(overlaps(library, transport)).toBe(false)
      const windows = await page.locator('.camera-window').evaluateAll((elements) => elements.map((element) => element.getBoundingClientRect().bottom))
      expect(Math.max(...windows)).toBeLessThanOrEqual(transport.y + 1)
    }

    const frames = await page.locator('.camera-window video').evaluateAll((videos) => videos.map((video) => {
      if (!(video instanceof HTMLVideoElement)) throw new Error('camera frame is not a video')
      return { fit: getComputedStyle(video).objectFit, width: video.videoWidth, height: video.videoHeight }
    }))
    expect(frames.every((frame) => frame.fit === 'contain' && frame.width > 0 && frame.height > 0)).toBe(true)
    const videoBackgrounds = await page.locator('.camera-window:visible').evaluateAll((elements) => elements.map((element) => getComputedStyle(element).backgroundColor))
    expect(videoBackgrounds.every((color) => relativeLuminance(color) < 0.05)).toBe(true)
    const floating = await page.locator('.transport, .pill').evaluateAll((elements) => elements.map((element) => getComputedStyle(element).backgroundColor))
    expect(floating.every((color) => relativeLuminance(color) > 0.75)).toBe(true)
    const play = await page.locator('.transport__play').evaluate((element) => getComputedStyle(element).backgroundColor)
    expect(contrastRatio(play, 'rgb(255, 255, 255)')).toBeGreaterThanOrEqual(3)

    await page.mouse.move(0, 0)
    expect(await undersizedButtons(page, viewport.mode === 'desktop' ? 36 : 44)).toEqual([])
    expect(await hasNoHorizontalScroll(page)).toBe(true)
    expect(runtimeErrors).toEqual([])
    await context.close()
  })
}

test('keeps the welcome button and its label on one line at 320 px', async ({ browser }) => {
  const context = await browser.newContext({ viewport: { width: 320, height: 700 } })
  const page = await context.newPage()
  await page.goto('/')
  const action = page.getByRole('button', { name: 'フォルダを選ぶ', exact: true })

  expect(await textSpansSingleLine(action, 'フォルダを選ぶ')).toBe(true)
  const bounds = await action.boundingBox()
  expect(bounds?.height ?? 0).toBeGreaterThanOrEqual(44)
  expect(await hasNoHorizontalScroll(page)).toBe(true)
  await context.close()
})

test('turns off motion when reduced motion is requested', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 }, reducedMotion: 'reduce' })
  await installPicker(context, media, 'full')
  const page = await context.newPage()
  await page.goto('/')
  await page.getByRole('button', { name: 'フォルダを選ぶ', exact: true }).click()
  await eventRow(page, '2026-08-20 20:45:00').click()

  const durations = await page.locator('.camera-window, .camera-window video, .event-row, .transport__play').evaluateAll((elements) => elements.flatMap((element) => {
    const styles = getComputedStyle(element)
    return [styles.transitionDuration, styles.animationDuration]
  }))
  expect(durations.every((duration) => duration.split(',').every((value) => value.trim() === '0s'))).toBe(true)
  await context.close()
})
