import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'

import { expect, test } from '@playwright/test'

import {
  captureRuntimeErrors,
  evidenceRoot,
  eventRow,
  fixturePath,
  hasNoHorizontalScroll,
  installPicker,
  selectFolder,
  textSpansSingleLine,
  viewports,
} from './support/viewer-fixtures'

for (const viewport of viewports) {
  test(`names a missing camera at ${viewport.name}`, async ({ browser }) => {
    const media = [...await readFile(fixturePath)]
    const context = await browser.newContext({ viewport: { width: viewport.width, height: viewport.height } })
    await installPicker(context, media, 'missing')
    const page = await context.newPage()
    await page.goto('/')
    await selectFolder(page)

    const row = eventRow(page, '2026-08-20 20:35:00')
    await expect(row).toContainText('5カメラ')
    await expect(row).toContainText('右リピーターの記録がありません')
    await row.click()
    if (viewport.mode === 'phone') await page.getByRole('button', { name: '右リピーターを表示' }).click()
    const rightRepeater = page.locator('.camera-window[data-camera="right_repeater"]')
    await expect(rightRepeater).toHaveAttribute('data-status', 'missing')
    await expect(rightRepeater.getByText('記録がありません')).toBeVisible()
    await page.locator('.event-pill').click()
    await expect(page.getByRole('dialog', { name: 'イベント詳細' })).toContainText('5カメラ右リピーターの記録がありません')
    await page.screenshot({ path: `${evidenceRoot}/${viewport.name}-missing.png` })
    expect(await hasNoHorizontalScroll(page)).toBe(true)
    await context.close()
  })
}

test('falls back to event.mp4 when every camera is corrupt', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  const context = await browser.newContext({ viewport: { width: 1280, height: 800 } })
  await installPicker(context, media, 'corrupt')
  const page = await context.newPage()
  const runtimeErrors = captureRuntimeErrors(page)
  await page.goto('/')
  await selectFolder(page)
  await eventRow(page, '2026-08-20 20:30:00').click()

  await expect(page.locator('.camera-window[data-status="corrupt"], .camera-window[data-status="unsupported"]')).toHaveCount(6)
  await expect(page.locator('.stage')).toHaveAttribute('data-mode', 'representative')
  await expect(page.getByRole('radio', { name: '代表映像' })).toBeChecked()
  await expect(page.getByRole('radio', { name: '車両配置' })).toBeDisabled()
  const representative = page.getByRole('region', { name: '代表映像' })
  await expect(representative.getByText('代表映像（単独再生）')).toBeVisible()
  expect(await textSpansSingleLine(representative.getByText('6カメラ同期映像ではありません。'), 'ありません。')).toBe(true)
  await expect(representative.locator('video')).toHaveCount(1)

  const geometry = await page.evaluate(() => {
    const rect = (selector: string) => {
      const element = document.querySelector(selector)
      if (element === null) throw new Error(`${selector} is missing`)
      return element.getBoundingClientRect()
    }
    const windows = [...document.querySelectorAll('.camera-window')].map((element) => element.getBoundingClientRect())
    const overlaps = [...document.querySelectorAll('.camera-window')].filter((element) => {
      const label = element.querySelector('.camera-window__label')?.getBoundingClientRect()
      const text = element.querySelector('.camera-window__state-text')
      if (label === undefined || !(text instanceof HTMLElement) || text.offsetParent === null) return false
      const state = text.getBoundingClientRect()
      return label.left < state.right && label.right > state.left && label.top < state.bottom && label.bottom > state.top
    }).length
    return {
      representativeBottom: rect('.representative').bottom,
      windowTop: Math.min(...windows.map((bounds) => bounds.top)),
      windowBottom: Math.max(...windows.map((bounds) => bounds.bottom)),
      transportTop: rect('.transport').top,
      transportBottom: rect('.transport').bottom,
      height: window.innerHeight,
      overlaps,
    }
  })
  expect(geometry.representativeBottom).toBeLessThanOrEqual(geometry.windowTop)
  expect(geometry.windowBottom).toBeLessThanOrEqual(geometry.transportTop)
  expect(geometry.transportBottom).toBeLessThanOrEqual(geometry.height)
  expect(geometry.overlaps).toBe(0)
  await page.screenshot({ path: `${evidenceRoot}/1280x800-representative.png` })
  expect(runtimeErrors).toEqual([])
  await context.close()
})

test('handles wrong folders, permission denial, cancellation and unknown picker failures', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  const scenarios = [
    { variant: 'wrong', expected: 'TeslaCamフォルダを確認できませんでした。' },
    { variant: 'denied', expected: 'フォルダへのアクセスが許可されませんでした。' },
    { variant: 'primitive', expected: 'フォルダの読み取りを完了できませんでした。' },
  ] as const
  for (const scenario of scenarios) {
    const context = await browser.newContext()
    await installPicker(context, media, scenario.variant)
    const page = await context.newPage()
    const runtimeErrors = captureRuntimeErrors(page)
    await page.goto('/')
    await page.getByRole('button', { name: 'フォルダを選ぶ', exact: true }).click()
    await expect(page.getByRole('alert')).toContainText(scenario.expected)
    await expect(page.getByRole('button', { name: 'もう一度選ぶ' })).toBeVisible()
    expect(runtimeErrors).toEqual([])
    await context.close()
  }
  const cancelledContext = await browser.newContext()
  await installPicker(cancelledContext, media, 'cancelled')
  const cancelledPage = await cancelledContext.newPage()
  await cancelledPage.goto('/')
  await cancelledPage.getByRole('button', { name: 'フォルダを選ぶ', exact: true }).click()
  await expect(cancelledPage.getByRole('heading', { name: 'TeslaCamの記録を見る' })).toBeVisible()
  await expect(cancelledPage.getByRole('alert')).toHaveCount(0)
  await cancelledContext.close()
})

test('keeps the loaded library when choosing another folder is cancelled', async ({ context, page }) => {
  const media = [...await readFile(fixturePath)]
  await installPicker(context, media, 'full')
  await page.goto('/')
  await selectFolder(page)
  await eventRow(page, '2026-08-20 20:45:00').click()
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()

  await page.evaluate(() => {
    Object.defineProperty(window, 'showDirectoryPicker', {
      configurable: true,
      value: async () => {
        throw new DOMException('', 'AbortError')
      },
    })
  })
  await page.getByRole('button', { name: '別のフォルダを選ぶ' }).click()

  await expect(eventRow(page, '2026-08-20 20:45:00')).toHaveAttribute('aria-pressed', 'true')
  await expect(page.locator('video')).toHaveCount(6)
  await expect(page.getByRole('heading', { name: 'TeslaCamの記録を見る' })).toHaveCount(0)
})

test('uses the real directory-input fallback with webkitRelativePath', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'showDirectoryPicker', { configurable: true, value: undefined })
  })
  await page.goto('/')
  await expect(page.getByRole('status', { name: 'ブラウザ互換性' })).toContainText('互換モード')
  const chooser = page.waitForEvent('filechooser')
  await page.getByRole('button', { name: 'フォルダを選ぶ', exact: true }).click()
  const fileChooser = await chooser
  await fileChooser.setFiles(fileURLToPath(new URL('./fixtures/TeslaCam', import.meta.url)))
  await expect(page.getByRole('button', { name: '別のフォルダを選ぶ' })).toBeVisible()
  await expect(page.getByRole('radio', { name: 'すべて 1件' })).toBeChecked()
  await expect(page.getByText('対応外のファイル 1 件を除外しました。')).toBeVisible()
})
