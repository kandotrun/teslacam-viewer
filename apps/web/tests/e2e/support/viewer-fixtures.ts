import { readFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'

import { expect, type BrowserContext, type Locator, type Page } from '@playwright/test'

export const fixturePath = fileURLToPath(new URL('../fixtures/TeslaCam/RecentClips/2026-08-20_20-45-00-front.mp4', import.meta.url))
export const sentryMetadataPath = fileURLToPath(new URL('../fixtures/TeslaCam/SentryClips/event-sentry/event.json', import.meta.url))
export const sentryMediaPath = fileURLToPath(new URL('../fixtures/TeslaCam/SentryClips/event-sentry/sentry-autoplay-20s.mp4', import.meta.url))
export const evidenceRoot = '.omo/evidence/teslacam-viewer/screenshots'
const cameras = ['front', 'back', 'left_pillar', 'left_repeater', 'right_pillar', 'right_repeater'] as const
export const viewports = [
  { name: '1280', mode: 'desktop', width: 1280, height: 1000 },
  { name: '768', mode: 'tablet', width: 768, height: 1024 },
  { name: '375', mode: 'mobile', width: 375, height: 844 },
] as const

type PickerVariant = 'full' | 'history' | 'export' | 'missing' | 'corrupt' | 'wrong' | 'denied' | 'cancelled' | 'primitive'

function colorChannels(color: string): readonly number[] {
  const hex = /^#([a-f\d]{2})([a-f\d]{2})([a-f\d]{2})$/iu.exec(color)
  if (hex !== null) return hex.slice(1).map((channel) => Number.parseInt(channel, 16) / 255)
  const rgb = color.match(/\d+(?:\.\d+)?/gu)
  if (rgb === null || rgb.length < 3) throw new Error('invalid color token')
  return rgb.slice(0, 3).map((channel) => Number(channel) / 255)
}

export function relativeLuminance(color: string): number {
  const channels = colorChannels(color).map((channel) => (
    channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4
  ))
  const red = channels.at(0)
  const green = channels.at(1)
  const blue = channels.at(2)
  if (red === undefined || green === undefined || blue === undefined) throw new Error('invalid color token')
  return (0.2126 * red) + (0.7152 * green) + (0.0722 * blue)
}

export function contrastRatio(firstColor: string, secondColor: string): number {
  const values = [relativeLuminance(firstColor), relativeLuminance(secondColor)].sort((first, second) => second - first)
  const lighter = values.at(0)
  const darker = values.at(1)
  if (lighter === undefined || darker === undefined) throw new Error('invalid contrast colors')
  return (lighter + 0.05) / (darker + 0.05)
}

export async function installPicker(context: BrowserContext, bytes: readonly number[], variant: PickerVariant): Promise<void> {
  const [sentryMetadata, sentryMedia] = await Promise.all([
    readFile(sentryMetadataPath, 'utf8'),
    readFile(sentryMediaPath),
  ])
  await context.addInitScript(({ mediaBytes, pickerVariant, cameraNames, metadataContents, sentryMediaBytes }) => {
    const media = new Uint8Array(mediaBytes)
    const sentryMedia = new Uint8Array(sentryMediaBytes)
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
    const previousSaved = directory('event-saved-previous', [
      ...clips('2026-08-19_19-30-00', cameraNames),
      file('event.mp4'),
    ])
    const sentryCameras = pickerVariant === 'missing' ? cameraNames.slice(0, 5) : cameraNames
    const sentry = directory('event-sentry', [
      ...clips('2026-08-20_20-35-00', sentryCameras, sentryMedia),
      file('event.json', new TextEncoder().encode(metadataContents)),
    ])
    const exportEvent = directory('event-export', [
      ...clips('2026-08-20_20-10-00', cameraNames),
      ...clips('2026-08-20_20-10-02', cameraNames),
      ...clips('2026-08-20_20-10-04', cameraNames),
      ...clips('2026-08-20_20-10-06', cameraNames),
      ...clips('2026-08-20_20-10-08', cameraNames),
    ])
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
    const historyRoot = directory('TeslaCam', [
      directory('Noise', syntheticNoise),
      recent,
      directory('SavedClips', [saved, previousSaved]),
      directory('SentryClips', [sentry]),
    ])
    const exportRoot = directory('TeslaCam', [
      directory('SavedClips', [exportEvent]),
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
          case 'history':
            return historyRoot
          case 'export':
            return exportRoot
          case 'full':
          case 'missing':
            return fullRoot
        }
      },
    })
  }, {
    mediaBytes: bytes,
    pickerVariant: variant,
    cameraNames: cameras,
    metadataContents: sentryMetadata,
    sentryMediaBytes: [...sentryMedia],
  })
}

export function captureRuntimeErrors(page: Page): readonly string[] {
  const errors: string[] = []
  page.on('pageerror', (error) => errors.push(error.message))
  page.on('console', (message) => {
    if (message.type() === 'error') errors.push(message.text())
  })
  return errors
}

export async function selectFolder(page: Page): Promise<void> {
  await page.getByRole('button', { name: 'TeslaCamフォルダを選ぶ', exact: true }).click()
  await expect(page.getByRole('button', { name: '別のフォルダを選ぶ' })).toBeVisible()
}

export async function textSpansSingleLine(locator: Locator, phrase: string): Promise<boolean> {
  return locator.evaluate((element, target) => {
    const walker = document.createTreeWalker(element, NodeFilter.SHOW_TEXT)
    let node = walker.nextNode()
    while (node !== null) {
      const content = node.textContent ?? ''
      const start = content.indexOf(target)
      if (start >= 0) {
        const range = document.createRange()
        range.setStart(node, start)
        range.setEnd(node, start + target.length)
        const lineTops = new Set([...range.getClientRects()].map((rect) => Math.round(rect.top)))
        return lineTops.size === 1
      }
      node = walker.nextNode()
    }
    return false
  }, phrase)
}
