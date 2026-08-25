import { readFile, stat } from 'node:fs/promises'

import { expect, test } from '@playwright/test'

import { captureRuntimeErrors, evidenceRoot, fixturePath, installPicker, selectFolder } from './support/viewer-fixtures'

test('exports the selected event as one local vehicle-layout video', async ({ context, page }) => {
  const media = [...await readFile(fixturePath)]
  await installPicker(context, media, 'full')
  const runtimeErrors = captureRuntimeErrors(page)
  const requests: string[] = []
  page.on('request', (request) => {
    if (/^https?:/u.test(request.url())) requests.push(`${request.url()} ${request.postData() ?? ''}`)
  })
  await page.goto('/')
  await selectFolder(page)
  await page.locator('.event-list').getByRole('button', { name: '2026-08-20 20:45:00', exact: true }).click()

  await page.getByRole('button', { name: '合成動画を書き出す' }).click()
  const dialog = page.getByRole('dialog', { name: '合成動画を書き出す' })
  await expect(dialog).toBeVisible()
  await expect(dialog.getByText(/MP4|WEBM/u)).toBeVisible()
  await page.screenshot({ path: `${evidenceRoot}/composite-export-dialog-1280.png` })
  await dialog.getByRole('button', { name: '書き出しを開始' }).click()

  await expect(page.getByText('書き出し中', { exact: true })).toBeVisible()
  await expect(page.getByText(/書き出し完了 · (MP4|WEBM) · 1280 × 720/u)).toBeVisible({ timeout: 20_000 })
  const saveButton = page.getByRole('button', { name: /^(MP4|WEBM)を保存$/u })
  const downloadPromise = page.waitForEvent('download')
  await saveButton.click()
  const download = await downloadPromise
  const extension = download.suggestedFilename().split('.').at(-1)
  expect(extension === 'mp4' || extension === 'webm').toBe(true)
  if (extension !== 'mp4' && extension !== 'webm') throw new Error('unexpected export extension')
  const outputPath = `${evidenceRoot}/composite-export.${extension}`
  await download.saveAs(outputPath)
  expect((await stat(outputPath)).size).toBeGreaterThan(1_000)
  const bytes = Array.from(await readFile(outputPath))
  const metadata = await page.evaluate(async ({ bytes: exportedBytes, mimeType }) => {
    const buffer = Uint8Array.from(exportedBytes).buffer
    const url = URL.createObjectURL(new Blob([buffer], { type: mimeType }))
    const video = document.createElement('video')
    try {
      return await new Promise<{ width: number; height: number; duration: number; letterboxPixel: readonly number[] }>((resolve, reject) => {
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
          resolve({
            width: video.videoWidth,
            height: video.videoHeight,
            duration: video.duration,
            letterboxPixel: [...context.getImageData(410, 100, 1, 1).data],
          })
        }, { once: true })
        video.addEventListener('error', () => reject(new Error('exported video did not decode')), { once: true })
        video.src = url
        video.load()
      })
    } finally {
      URL.revokeObjectURL(url)
    }
  }, {
    bytes,
    mimeType: extension === 'mp4' ? 'video/mp4' : 'video/webm',
  })
  expect(metadata).toMatchObject({ width: 1280, height: 720 })
  expect(metadata.duration).toBeGreaterThan(0)
  expect(Math.max(...metadata.letterboxPixel.slice(0, 3))).toBeLessThanOrEqual(8)

  const pageOrigin = new URL(page.url()).origin
  expect(requests.every((request) => request.startsWith(pageOrigin))).toBe(true)
  expect(requests.every((request) => !request.includes('2026-08-20_20-45-00'))).toBe(true)
  expect(runtimeErrors).toEqual([])
})

test('exports a normal-speed composite faster than its ten-second runtime', async ({ context, page }) => {
  const media = [...await readFile(fixturePath)]
  await installPicker(context, media, 'export')
  await page.goto('/')
  await selectFolder(page)
  await page.locator('.event-list').getByRole('button', { name: '2026-08-20 20:10:00', exact: true }).click()

  await page.getByRole('button', { name: '合成動画を書き出す' }).click()
  const dialog = page.getByRole('dialog', { name: '合成動画を書き出す' })
  const startedAt = performance.now()
  await dialog.getByRole('button', { name: '書き出しを開始' }).click()
  await expect(page.getByText(/書き出し完了 · (MP4|WEBM) · 1280 × 720/u)).toBeVisible({ timeout: 20_000 })
  const elapsedSeconds = (performance.now() - startedAt) / 1000

  const saveButton = page.getByRole('button', { name: /^(MP4|WEBM)を保存$/u })
  const downloadPromise = page.waitForEvent('download')
  await saveButton.click()
  const download = await downloadPromise
  const extension = download.suggestedFilename().split('.').at(-1)
  if (extension !== 'mp4' && extension !== 'webm') throw new Error('unexpected export extension')
  const outputPath = `${evidenceRoot}/composite-export-speed.${extension}`
  await download.saveAs(outputPath)
  const bytes = Array.from(await readFile(outputPath))
  const duration = await page.evaluate(async ({ bytes: exportedBytes, mimeType }) => {
    const url = URL.createObjectURL(new Blob([Uint8Array.from(exportedBytes)], { type: mimeType }))
    const video = document.createElement('video')
    try {
      return await new Promise<number>((resolve, reject) => {
        video.addEventListener('loadedmetadata', () => resolve(video.duration), { once: true })
        video.addEventListener('error', () => reject(new Error('exported video did not decode')), { once: true })
        video.src = url
        video.load()
      })
    } finally {
      URL.revokeObjectURL(url)
    }
  }, {
    bytes,
    mimeType: extension === 'mp4' ? 'video/mp4' : 'video/webm',
  })

  expect(duration).toBeGreaterThan(9.5)
  expect(elapsedSeconds).toBeLessThan(duration * 0.9)
})

test('keeps the export confirmation usable on mobile and restores focus', async ({ browser }) => {
  const media = [...await readFile(fixturePath)]
  const context = await browser.newContext({ viewport: { width: 375, height: 844 } })
  await installPicker(context, media, 'full')
  const page = await context.newPage()
  await page.goto('/')
  await selectFolder(page)
  await page.locator('.event-list').getByRole('button', { name: '2026-08-20 20:45:00', exact: true }).click()

  const trigger = page.getByRole('button', { name: '合成動画を書き出す' })
  await trigger.click()
  const dialog = page.getByRole('dialog', { name: '合成動画を書き出す' })
  await expect(dialog).toBeVisible()
  const bounds = await dialog.boundingBox()
  if (bounds === null) throw new Error('export dialog geometry is missing')
  expect(bounds.x).toBeGreaterThanOrEqual(0)
  expect(bounds.x + bounds.width).toBeLessThanOrEqual(375)
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
  await page.screenshot({ path: `${evidenceRoot}/composite-export-dialog-375.png` })

  await page.keyboard.press('Escape')
  await expect(dialog).toBeHidden()
  await expect(trigger).toBeFocused()
  await context.close()
})
