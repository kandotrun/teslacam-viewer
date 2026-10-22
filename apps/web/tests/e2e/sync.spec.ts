import { expect, test, type Page } from '@playwright/test'

import {
  after,
  frameSpread,
  recordSyncMetric,
  sampleCameraFrames,
  segmentFirstFrame,
  summarizeSpread,
  waitForAllCamerasPlaying,
  type FrameSample,
} from './support/frame-counter'
import { eventRow, installPicker, selectFolder } from './support/viewer-fixtures'

// Every camera of the sync event plays the same frame-counter clip, so a frame-accurate player shows
// the same counter in all six windows. One frame is 1/36 s (≈ 28 ms). Faster than 1× the screen skips
// frames, and one display refresh (≈ 17 ms) already spans more than one frame at 2×.
const maxSpreadFrames = 1
const maxSpreadFramesAt2x = 2

test.beforeEach(async ({ context }) => {
  await installPicker(context, [], 'sync')
})

async function openSyncEvent(page: Page): Promise<void> {
  await page.goto('/')
  await selectFolder(page)
  await eventRow(page, '2026-08-20 20:50:00').click()
  await expect(page.locator('.camera-window[data-status="ready"]')).toHaveCount(6)
  await waitForAllCamerasPlaying(page)
}

async function seekTo(page: Page, seconds: number): Promise<void> {
  await page.getByLabel('再生位置').fill(String(seconds))
  await waitForAllCamerasPlaying(page)
}

test('keeps the six cameras within one frame while playing and after a seek', async ({ page }) => {
  await openSyncEvent(page)
  const playing = summarizeSpread(after(await sampleCameraFrames(page, 2_000), 500))
  expect(playing).toMatchObject({ blank: 0 })
  expect(playing.maxSpread, JSON.stringify(playing)).toBeLessThanOrEqual(maxSpreadFrames)

  await seekTo(page, 1)
  const seeked = summarizeSpread(after(await sampleCameraFrames(page, 2_000), 500))
  expect(seeked).toMatchObject({ blank: 0 })
  expect(seeked.maxSpread, JSON.stringify(seeked)).toBeLessThanOrEqual(maxSpreadFrames)
  await recordSyncMetric('playing 1x', playing)
  await recordSyncMetric('after a seek', seeked)
})

test('keeps the six cameras within one display refresh at 2×', async ({ page }) => {
  await openSyncEvent(page)
  await page.getByLabel('再生速度').selectOption('2')
  const fast = summarizeSpread(after(await sampleCameraFrames(page, 2_000), 500))
  expect(fast).toMatchObject({ blank: 0 })
  expect(fast.maxSpread, JSON.stringify(fast)).toBeLessThanOrEqual(maxSpreadFramesAt2x)
  await recordSyncMetric('playing 2x', fast)
})

test('pulls a camera that drifted 0.2 s ahead back within one frame', async ({ page }) => {
  await openSyncEvent(page)
  await page.locator('.camera-window[data-camera="back"] video[data-testid="camera-video"]').evaluate((video) => {
    if (!(video instanceof HTMLVideoElement)) throw new Error('camera frame is not a video')
    video.currentTime += 0.2
  })
  const samples = await sampleCameraFrames(page, 3_000)
  const settledAt = samples.findLastIndex((sample) => (frameSpread(sample) ?? Number.POSITIVE_INFINITY) > maxSpreadFrames)
  const settled = summarizeSpread(after(samples, 2_000))
  expect(settled.maxSpread, JSON.stringify(settled)).toBeLessThanOrEqual(maxSpreadFrames)
  await recordSyncMetric('recovery from 0.2 s drift (ms)', Math.round(samples.at(settledAt + 1)?.at ?? Number.NaN))
})

function segmentOf(frame: number | null): number | null {
  return frame === null ? null : Math.floor(frame / segmentFirstFrame(1))
}

function boundaryReport(samples: readonly FrameSample[]): {
  readonly blank: number
  readonly mixed: number
  readonly stallMs: number
  readonly firstNextSegmentAt: number
} {
  const blank = samples.filter((sample) => sample.frames.some((frame) => frame === null)).length
  const mixed = samples.filter((sample) => new Set(sample.frames.map(segmentOf)).size > 1).length
  const lastBefore = samples.findLast((sample) => (sample.frames[0] ?? 0) < segmentFirstFrame(1))
  const moving = samples.find((sample) => (sample.frames[0] ?? 0) > segmentFirstFrame(1))
  const firstNext = samples.find((sample) => sample.frames.every((frame) => segmentOf(frame) === 1))
  return {
    blank,
    mixed,
    stallMs: Math.round((moving?.at ?? Number.NaN) - (lastBefore?.at ?? Number.NaN)),
    firstNextSegmentAt: firstNext?.at ?? Number.POSITIVE_INFINITY,
  }
}

test('crosses into the next segment without a blank window and stays within one frame', async ({ page }) => {
  await openSyncEvent(page)
  await seekTo(page, 6.5)
  const samples = await sampleCameraFrames(page, 3_500)
  const boundary = boundaryReport(samples)
  expect(boundary, JSON.stringify(boundary)).toMatchObject({ blank: 0 })
  expect(boundary.mixed, JSON.stringify(boundary)).toBeLessThanOrEqual(1)
  expect(boundary.stallMs, JSON.stringify(boundary)).toBeLessThan(100)
  const nextSegment = summarizeSpread(after(samples, boundary.firstNextSegmentAt + 500))
  expect(nextSegment.samples).toBeGreaterThan(30)
  expect(nextSegment.maxSpread, JSON.stringify(nextSegment)).toBeLessThanOrEqual(maxSpreadFrames)
  await recordSyncMetric('segment boundary', { ...boundary, nextSegment })
})
