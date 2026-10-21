import { readFile, stat } from 'node:fs/promises'

import { expect, test, type Page } from '@playwright/test'

import { captureRuntimeErrors, evidenceRoot, eventRow, fixturePath, installPicker, sameOriginRequests, selectFolder } from './support/viewer-fixtures'

type DecodedExport = {
  readonly width: number
  readonly height: number
  readonly duration: number
  readonly floor: readonly number[]
  readonly frontVideo: readonly number[]
  readonly label: readonly number[]
}

async function decodeExport(page: Page, bytes: readonly number[], mimeType: string): Promise<DecodedExport> {
  return page.evaluate(async ({ exportedBytes, type }) => {
    const url = URL.createObjectURL(new Blob([Uint8Array.from(exportedBytes)], { type }))
    const video = document.createElement('video')
    try {
      return await new Promise<DecodedExport>((resolve, reject) => {
        video.addEventListener('loadedmetadata', () => {
          video.currentTime = Math.min(0.5, video.duration / 2)
        }, { once: true })
        video.addEventListener('seeked', () => {
          const canvas = document.createElement('canvas')
          canvas.width = video.videoWidth
          canvas.height = video.videoHeight
          const context = canvas.getContext('2d')
          if (context === null) {
            reject(new Error('exported frame context is missing'))
            return
          }
          context.drawImage(video, 0, 0)
          const pixel = (x: number, y: number) => [...context.getImageData(x, y, 1, 1).data]
          resolve({
            width: video.videoWidth,
            height: video.videoHeight,
            duration: video.duration,
            floor: pixel(20, 20),
            frontVideo: pixel(640, 144),
            label: pixel(480, 254),
          })
        }, { once: true })
        video.addEventListener('error', () => reject(new Error('exported video did not decode')), { once: true })
        video.src = url
        video.load()
      })
    } finally {
      URL.revokeObjectURL(url)
    }
  }, { exportedBytes: bytes, type: mimeType })
}

function near(actual: readonly number[], expected: readonly number[], tolerance: number): boolean {
  return expected.every((channel, index) => Math.abs((actual[index] ?? 0) - channel) <= tolerance)
}

test('exports the selected event as one garage-floor video on this device', async ({ context, page }) => {
  const media = [...await readFile(fixturePath)]
  await installPicker(context, media, 'full')
  const runtimeErrors = captureRuntimeErrors(page)
  const network = sameOriginRequests(page)
  await page.goto('/')
  await selectFolder(page)
  await eventRow(page, '2026-08-20 20:45:00').click()

  await page.getByRole('button', { name: '合成動画を書き出す' }).click()
  const dialog = page.getByRole('dialog', { name: '合成動画を書き出す' })
  await expect(dialog).toBeVisible()
  await expect(dialog).toContainText('1280 × 720')
  await expect(dialog.getByText(/^MP4/u)).toBeVisible()
  await expect(dialog.locator('svg.export-preview')).toBeVisible()
  await page.screenshot({ path: `${evidenceRoot}/1280x1000-export-dialog.png` })
  await dialog.getByRole('button', { name: '書き出しを開始' }).click()

  await expect(page.getByText('書き出し中', { exact: true })).toBeVisible()
  await expect(page.getByRole('status').filter({ hasText: '書き出し完了' })).toBeVisible({ timeout: 20_000 })
  const saveButton = page.getByRole('button', { name: /^(MP4|WEBM)を保存$/u })
  const downloadPromise = page.waitForEvent('download')
  await saveButton.click()
  const download = await downloadPromise
  const extension = download.suggestedFilename().split('.').at(-1)
  if (extension !== 'mp4' && extension !== 'webm') throw new Error('unexpected export extension')
  expect(download.suggestedFilename()).toBe(`TeslaCam-2026-08-20_20-45-00-vehicle.${extension}`)
  const outputPath = `${evidenceRoot}/composite-export.${extension}`
  await download.saveAs(outputPath)
  expect((await stat(outputPath)).size).toBeGreaterThan(1_000)

  const decoded = await decodeExport(page, [...await readFile(outputPath)], extension === 'mp4' ? 'video/mp4' : 'video/webm')
  expect(decoded).toMatchObject({ width: 1280, height: 720 })
  expect(decoded.duration).toBeGreaterThan(0)
  expect(near(decoded.floor, [0xE9, 0xEB, 0xEE], 8)).toBe(true)
  expect(near(decoded.frontVideo, [0xE9, 0xEB, 0xEE], 24)).toBe(false)

  const pageOrigin = new URL(page.url()).origin
  expect(network.requests.every((request) => request.startsWith(pageOrigin))).toBe(true)
  expect(network.requests.some((request) => request.includes('2026-08-20_20-45-00'))).toBe(false)
  expect(runtimeErrors).toEqual([])
})

test('exports a normal-speed composite faster than its ten-second runtime', async ({ context, page }) => {
  const media = [...await readFile(fixturePath)]
  await installPicker(context, media, 'export')
  await page.goto('/')
  await selectFolder(page)
  await eventRow(page, '2026-08-20 20:10:00').click()

  await page.getByRole('button', { name: '合成動画を書き出す' }).click()
  const dialog = page.getByRole('dialog', { name: '合成動画を書き出す' })
  const startedAt = performance.now()
  await dialog.getByRole('button', { name: '書き出しを開始' }).click()
  await expect(page.getByRole('status').filter({ hasText: '書き出し完了' })).toBeVisible({ timeout: 20_000 })
  const elapsedSeconds = (performance.now() - startedAt) / 1000

  const downloadPromise = page.waitForEvent('download')
  await page.getByRole('button', { name: /^(MP4|WEBM)を保存$/u }).click()
  const download = await downloadPromise
  const extension = download.suggestedFilename().split('.').at(-1)
  if (extension !== 'mp4' && extension !== 'webm') throw new Error('unexpected export extension')
  const outputPath = `${evidenceRoot}/composite-export-speed.${extension}`
  await download.saveAs(outputPath)
  const decoded = await decodeExport(page, [...await readFile(outputPath)], extension === 'mp4' ? 'video/mp4' : 'video/webm')

  expect(decoded.duration).toBeGreaterThan(9.5)
  expect(elapsedSeconds).toBeLessThan(decoded.duration * 0.9)
})

test('aborts an export without offering a file', async ({ context, page }) => {
  const media = [...await readFile(fixturePath)]
  await installPicker(context, media, 'export')
  await page.goto('/')
  await selectFolder(page)
  await eventRow(page, '2026-08-20 20:10:00').click()

  await page.getByRole('button', { name: '合成動画を書き出す' }).click()
  await page.getByRole('dialog', { name: '合成動画を書き出す' }).getByRole('button', { name: '書き出しを開始' }).click()
  await page.getByRole('button', { name: /^書き出し中/u }).click()
  await page.getByRole('button', { name: '中止' }).click()

  await expect(page.getByRole('status').filter({ hasText: '書き出しを中止しました。' })).toBeVisible()
  await expect(page.getByRole('button', { name: /を保存$/u })).toHaveCount(0)
})

test('keeps the export dialog usable on a phone and restores focus', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  const context = await browser.newContext({ viewport: { width: 375, height: 812 } })
  await installPicker(context, media, 'full')
  const page = await context.newPage()
  await page.goto('/')
  await selectFolder(page)
  await eventRow(page, '2026-08-20 20:45:00').click()

  const trigger = page.getByRole('button', { name: '合成動画を書き出す' })
  await trigger.click()
  const dialog = page.getByRole('dialog', { name: '合成動画を書き出す' })
  await expect(dialog).toBeVisible()
  const bounds = await dialog.boundingBox()
  if (bounds === null) throw new Error('export dialog geometry is missing')
  expect(bounds.x).toBeGreaterThanOrEqual(0)
  expect(bounds.x + bounds.width).toBeLessThanOrEqual(375)
  expect(bounds.y + bounds.height).toBeLessThanOrEqual(812)
  await page.screenshot({ path: `${evidenceRoot}/375x812-export-dialog.png` })

  await page.keyboard.press('Escape')
  await expect(dialog).toBeHidden()
  await expect(trigger).toBeFocused()
  await context.close()
})
