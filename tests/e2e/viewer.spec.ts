import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'

import { expect, test, type BrowserContext, type Page } from '@playwright/test'

const fixturePath = fileURLToPath(new URL('./fixtures/TeslaCam/RecentClips/2026-08-20_20-45-00-front.mp4', import.meta.url))
const evidenceRoot = '.omo/evidence/teslacam-viewer/screenshots'
const cameras = ['front', 'back', 'left_pillar', 'left_repeater', 'right_pillar', 'right_repeater'] as const
const viewports = [
  { name: 'desktop', width: 1440, height: 1000 },
  { name: 'tablet', width: 1024, height: 768 },
  { name: 'mobile', width: 390, height: 844 },
] as const

type PickerVariant = 'full' | 'missing' | 'corrupt' | 'wrong' | 'denied' | 'cancelled' | 'primitive'

async function installPicker(context: BrowserContext, bytes: readonly number[], variant: PickerVariant): Promise<void> {
  await context.addInitScript(({ mediaBytes, pickerVariant, cameraNames }) => {
    const media = new Uint8Array(mediaBytes)
    const file = (name: string, content: Uint8Array = media) => ({
      kind: 'file',
      name,
      getFile: async () => new File([content], name, { type: 'video/mp4' }),
    })
    const directory = (name: string, entries: readonly object[]) => ({
      kind: 'directory',
      name,
      values: async function* () {
        for (const entry of entries) yield entry
      },
    })
    const clips = (timestamp: string, selectedCameras: readonly string[], content: Uint8Array = media) =>
      selectedCameras.map((camera) => file(`${timestamp}-${camera}.mp4`, content))
    const saved = directory('event-saved', [
      ...clips('2026-08-20_20-40-00', cameraNames),
      file('event.mp4'),
    ])
    const sentryCameras = pickerVariant === 'missing' ? cameraNames.slice(0, 5) : cameraNames
    const sentry = directory('event-sentry', clips('2026-08-20_20-35-00', sentryCameras))
    const recent = directory('RecentClips', clips('2026-08-20_20-45-00', cameraNames))
    const syntheticNoise = Array.from({ length: 253 }, (_, index) => index === 248
      ? {
          kind: 'file',
          name: `synthetic-${index}.txt`,
          getFile: async () => new Promise<File>((resolve) => {
            setTimeout(() => resolve(new File([new Uint8Array([1])], `synthetic-${index}.txt`)), 150)
          }),
        }
      : file(`synthetic-${index}.txt`, new Uint8Array([1])))
    const fullRoot = directory('TeslaCam', [
      directory('Noise', syntheticNoise),
      recent,
      directory('SavedClips', [saved]),
      directory('SentryClips', [sentry]),
    ])
    const corruptRoot = directory('TeslaCam', [
      directory('SavedClips', [directory('event-corrupt', [
        ...clips('2026-08-20_20-30-00', cameraNames, new Uint8Array([1, 2, 3, 4])),
        file('event.mp4'),
      ])]),
    ])
    const wrongRoot = directory('SyntheticFolder', [file('notes.txt', new Uint8Array([1]))])
    Object.defineProperty(window, 'showDirectoryPicker', {
      configurable: true,
      value: async () => {
        switch (pickerVariant) {
          case 'denied':
            throw new DOMException('', 'NotAllowedError')
          case 'cancelled':
            throw new DOMException('', 'AbortError')
          case 'primitive':
            throw 'picker unavailable'
          case 'wrong':
            return wrongRoot
          case 'corrupt':
            return corruptRoot
          case 'full':
          case 'missing':
            return fullRoot
        }
      },
    })
  }, { mediaBytes: bytes, pickerVariant: variant, cameraNames: cameras })
}

function captureRuntimeErrors(page: Page): readonly string[] {
  const errors: string[] = []
  page.on('pageerror', (error) => errors.push(error.message))
  page.on('console', (message) => {
    if (message.type() === 'error') errors.push(message.text())
  })
  return errors
}

async function selectFolder(page: Page): Promise<void> {
  await page.getByRole('button', { name: 'TeslaCamフォルダを選ぶ', exact: true }).click()
  await expect(page.getByRole('button', { name: '別のフォルダを選ぶ' })).toBeVisible()
}

test('captures welcome and loaded states at the required viewports', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  for (const viewport of viewports) {
    const context = await browser.newContext({ viewport: { width: viewport.width, height: viewport.height } })
    await installPicker(context, media, 'full')
    const page = await context.newPage()
    const runtimeErrors = captureRuntimeErrors(page)
    await page.goto('/')
    await expect(page.getByRole('heading', { name: 'TeslaCamを、ひとつのシアターで。' })).toBeVisible()
    const welcomePhraseLineCount = await page.locator('.welcome-title__phrase').evaluate((element) => element.getClientRects().length)
    expect(welcomePhraseLineCount).toBe(1)
    if (viewport.name === 'mobile') {
      const mobileTypography = await page.getByRole('heading', { name: 'TeslaCamを、ひとつのシアターで。' }).evaluate((element) => ({
        computed: Number.parseFloat(getComputedStyle(element).fontSize),
        root: Number.parseFloat(getComputedStyle(document.documentElement).fontSize),
        token: getComputedStyle(document.documentElement).getPropertyValue('--font-display-mobile').trim(),
      }))
      expect(mobileTypography.token).not.toBe('')
      expect(mobileTypography.computed).toBe(Number.parseFloat(mobileTypography.token) * mobileTypography.root)
    }
    await page.screenshot({ path: `${evidenceRoot}/${viewport.name}-welcome.png` })
    const selection = page.getByRole('button', { name: 'TeslaCamフォルダを選ぶ', exact: true })
    await selection.focus()
    await expect(selection).toBeFocused()
    await selection.click()
    await expect(page.getByText(/250 ファイルを確認中です/)).toBeVisible()
    await expect(page.getByRole('button', { name: '別のフォルダを選ぶ' })).toBeVisible()
    if (viewport.name === 'tablet') {
      await expect(page.getByRole('button', { name: 'イベント一覧を閉じる' })).toBeVisible()
    }
    await page.getByRole('button', { name: /2026-08-20 20:45:00/ }).click()
    await expect(page.locator('video')).toHaveCount(6)
    if (viewport.name !== 'desktop') {
      await expect(page.getByRole('button', { name: 'イベント一覧を開く' })).toHaveAttribute('aria-expanded', 'false')
      const activeCamera = await page.locator('.camera-tile[data-active="true"]').boundingBox()
      const transport = await page.locator('.playback-controls').boundingBox()
      if (activeCamera === null || transport === null) throw new Error('responsive theater geometry is missing')
      expect(activeCamera.y + activeCamera.height).toBeLessThanOrEqual(transport.y)
    }
    if (viewport.name === 'tablet') {
      const maximumControlHeight = await page.locator('.playback-controls__primary button').evaluateAll((elements) => Math.max(...elements.map((element) => element.getBoundingClientRect().height)))
      expect(maximumControlHeight).toBeLessThanOrEqual(48)
    }
    await page.screenshot({ path: `${evidenceRoot}/${viewport.name}-loaded.png` })
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
    expect(runtimeErrors).toEqual([])
    await context.close()
  }
})

test('drives categories, playback, camera and event keyboard controls without an HTTP leak', async ({ context, page }) => {
  const media = [...await readFile(fixturePath)]
  await installPicker(context, media, 'full')
  const runtimeErrors = captureRuntimeErrors(page)
  const requests: string[] = []
  page.on('request', (request) => {
    if (/^https?:/u.test(request.url())) requests.push(`${request.url()} ${request.postData() ?? ''}`)
  })
  await page.goto('/')
  await selectFolder(page)
  const categories = [
    { name: /^すべて\s*3$/u, count: '3 件' },
    { name: /^最近の走行\s*1$/u, count: '1 件' },
    { name: /^保存済み\s*1$/u, count: '1 件' },
    { name: /^セントリー\s*1$/u, count: '1 件' },
  ] as const
  for (const category of categories) {
    await page.getByRole('button', { name: category.name }).click()
    await expect(page.getByText(category.count, { exact: true })).toBeVisible()
  }
  await page.getByRole('button', { name: /^すべて\s*3$/u }).click()
  await page.getByRole('button', { name: /2026-08-20 20:45:00/ }).click()
  await expect(page.locator('video')).toHaveCount(6)
  await expect(page.locator('.camera-tile[data-status="ready"]')).toHaveCount(6)
  await page.getByRole('button', { name: '再生' }).click()
  await expect(page.getByRole('button', { name: '一時停止' })).toBeVisible()
  await expect.poll(async () => Number(await page.getByLabel('再生位置').inputValue())).toBeGreaterThan(0)
  await page.evaluate(() => {
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur()
  })
  await page.keyboard.press('Space')
  await expect(page.getByRole('button', { name: '再生' })).toBeVisible()
  await page.keyboard.press('ArrowRight')
  expect(Number(await page.getByLabel('再生位置').inputValue())).toBeGreaterThan(1)
  await page.keyboard.press('Digit6')
  await expect(page.getByRole('button', { name: '右リピーターカメラをグリッドへ戻す' })).toBeVisible()
  await page.keyboard.press('Escape')
  await expect(page.getByRole('button', { name: '右リピーターカメラを拡大' })).toBeVisible()
  await page.getByRole('button', { name: '次のイベント' }).click()
  await expect(page.getByRole('button', { name: /2026-08-20 20:40:00/ })).toHaveAttribute('aria-pressed', 'true')
  await page.getByRole('button', { name: '前のイベント' }).click()
  const visibleControls = page.locator('button:visible')
  const sizes = await visibleControls.evaluateAll((elements) => elements.map((element) => {
    const bounds = element.getBoundingClientRect()
    return { width: bounds.width, height: bounds.height }
  }))
  expect(sizes.every((size) => size.width >= 44 && size.height >= 44)).toBe(true)
  expect(requests.some((request) => request.includes('2026-08-20_20') || request.includes('synthetic-') || request.includes('ftyp'))).toBe(false)
  expect(runtimeErrors).toEqual([])
})

test('shows missing and terminal media states with representative fallback', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  const missingContext = await browser.newContext({ viewport: { width: 1440, height: 1000 } })
  await installPicker(missingContext, media, 'missing')
  const missingPage = await missingContext.newPage()
  await missingPage.goto('/')
  await selectFolder(missingPage)
  await missingPage.getByRole('button', { name: /2026-08-20 20:35:00/ }).click()
  await expect(missingPage.getByText('映像がありません')).toBeVisible()
  await missingPage.screenshot({ path: `${evidenceRoot}/desktop-missing.png` })
  await missingContext.close()

  const corruptContext = await browser.newContext({ viewport: { width: 1440, height: 1000 } })
  await installPicker(corruptContext, media, 'corrupt')
  const corruptPage = await corruptContext.newPage()
  await corruptPage.goto('/')
  await selectFolder(corruptPage)
  await corruptPage.getByRole('button', { name: /2026-08-20 20:30:00/ }).click()
  await expect(corruptPage.locator('.camera-tile[data-status="corrupt"], .camera-tile[data-status="unsupported"]')).toHaveCount(6)
  await expect(corruptPage.getByText('代表映像（単独再生）')).toBeVisible()
  const terminalGrid = await corruptPage.locator('.camera-grid').boundingBox()
  const representative = await corruptPage.locator('.representative-video').boundingBox()
  const transport = await corruptPage.locator('.playback-controls').boundingBox()
  if (terminalGrid === null || representative === null || transport === null) throw new Error('terminal layout geometry is missing')
  const maximumTileBottom = await corruptPage.locator('.camera-tile').evaluateAll((elements) => Math.max(...elements.map((element) => {
    const bounds = element.getBoundingClientRect()
    return bounds.y + bounds.height
  })))
  const internalOverlapCount = await corruptPage.locator('.camera-tile').evaluateAll((elements) => elements.filter((element) => {
    const label = element.querySelector('.camera-tile__label')
    const state = element.querySelector('.camera-tile__state')
    if (label === null || state === null) return false
    const labelBounds = label.getBoundingClientRect()
    const stateBounds = state.getBoundingClientRect()
    return labelBounds.left < stateBounds.right && labelBounds.right > stateBounds.left && labelBounds.top < stateBounds.bottom && labelBounds.bottom > stateBounds.top
  }).length)
  expect(maximumTileBottom).toBeLessThanOrEqual(representative.y)
  expect(internalOverlapCount).toBe(0)
  expect(representative.y + representative.height).toBeLessThanOrEqual(transport.y)
  await corruptPage.screenshot({ path: `${evidenceRoot}/desktop-terminal-fallback.png` })
  await corruptContext.close()
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
    await page.getByRole('button', { name: 'TeslaCamフォルダを選ぶ', exact: true }).click()
    await expect(page.getByText(scenario.expected)).toBeVisible()
    expect(runtimeErrors).toEqual([])
    await context.close()
  }
  const cancelledContext = await browser.newContext()
  await installPicker(cancelledContext, media, 'cancelled')
  const cancelledPage = await cancelledContext.newPage()
  await cancelledPage.goto('/')
  await cancelledPage.getByRole('button', { name: 'TeslaCamフォルダを選ぶ', exact: true }).click()
  await expect(cancelledPage.getByRole('heading', { name: 'TeslaCamを、ひとつのシアターで。' })).toBeVisible()
  await cancelledContext.close()
})

test('uses the real directory-input fallback with webkitRelativePath', async ({ page }) => {
  await page.addInitScript(() => {
    Object.defineProperty(window, 'showDirectoryPicker', { configurable: true, value: undefined })
  })
  await page.goto('/')
  await expect(page.getByRole('status', { name: 'ブラウザ互換性' })).toContainText('互換モード')
  const chooser = page.waitForEvent('filechooser')
  await page.getByRole('button', { name: 'TeslaCamフォルダを選ぶ', exact: true }).click()
  const fileChooser = await chooser
  await fileChooser.setFiles(fileURLToPath(new URL('./fixtures/TeslaCam', import.meta.url)))
  await expect(page.getByRole('button', { name: '別のフォルダを選ぶ' })).toBeVisible()
  await expect(page.getByRole('button', { name: /^すべて\s*1$/u })).toBeVisible()
})

test('keeps responsive disclosures, mobile cameras, reduced motion and expanded evidence accurate', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, reducedMotion: 'reduce' })
  await installPicker(context, media, 'full')
  const page = await context.newPage()
  await page.goto('/')
  await selectFolder(page)
  const disclosure = page.getByRole('button', { name: 'イベント一覧を閉じる' })
  await expect(disclosure).toBeVisible()
  await disclosure.click()
  await expect(page.getByRole('button', { name: 'イベント一覧を開く' })).toHaveAttribute('aria-expanded', 'false')
  await page.getByRole('button', { name: 'イベント一覧を開く' }).click()
  await page.getByRole('button', { name: /2026-08-20 20:45:00/ }).click()
  await expect(page.getByRole('group', { name: '表示するカメラ' })).toBeVisible()
  const switcher = page.getByRole('group', { name: '表示するカメラ' })
  await expect(switcher.getByRole('button')).toHaveCount(6)
  expect(await switcher.evaluate((element) => element.scrollWidth > element.clientWidth)).toBe(true)
  await switcher.getByRole('button', { name: '右リピーターを表示' }).scrollIntoViewIfNeeded()
  await expect(switcher.getByRole('button', { name: '右リピーターを表示' })).toBeVisible()
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  await expect(page.locator('video')).toHaveCount(6)
  await expect(page.locator('.camera-tile:visible')).toHaveCount(1)
  await page.getByRole('button', { name: '右ピラーを表示' }).click()
  await expect(page.getByRole('button', { name: '右ピラーを表示' })).toHaveAttribute('aria-pressed', 'true')
  await page.getByRole('group', { name: '表示するカメラ' }).scrollIntoViewIfNeeded()
  const transitions = await page.locator('.camera-tile, .camera-switcher button, .playback-controls button').evaluateAll((elements) => elements.map((element) => getComputedStyle(element).transitionDuration))
  expect(transitions.every((duration) => duration === '0s')).toBe(true)
  await page.screenshot({ path: `${evidenceRoot}/mobile-switcher.png` })
  await page.setViewportSize({ width: 1440, height: 1000 })
  await page.getByRole('button', { name: '右ピラーカメラを拡大' }).click()
  await page.screenshot({ path: `${evidenceRoot}/desktop-expanded.png` })
  await context.close()
})
