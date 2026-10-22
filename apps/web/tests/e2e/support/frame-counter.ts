import { mkdir, readFile, writeFile } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'

import type { Page } from '@playwright/test'

/** The frame-counter clips of scripts/gen-sync-fixtures.sh, one per 60 s segment. */
export const syncSegmentPaths = [0, 1, 2].map((segment) => (
  fileURLToPath(new URL(`../fixtures/sync/segment-${segment}.mp4`, import.meta.url))
))
export const syncFps = 36
/** Event-wide frame index of the first frame of a segment (60 s per segment). */
export const segmentFirstFrame = (segment: number): number => segment * 60 * syncFps

const cameras = ['front', 'back', 'left_pillar', 'left_repeater', 'right_pillar', 'right_repeater'] as const

/** One read of all six playing videos within the same animation frame; `null` is a blank window. */
export type FrameSample = {
  readonly at: number
  readonly frames: readonly (number | null)[]
}

/**
 * Reads the burned-in counter of every camera's playing video on each animation frame for `durationMs`,
 * drawing the frame the browser presents onto a canvas (as a viewer would see it).
 */
export async function sampleCameraFrames(page: Page, durationMs: number): Promise<readonly FrameSample[]> {
  return page.evaluate(async ({ duration, cameraIds }) => {
    const canvas = document.createElement('canvas')
    canvas.width = 320
    canvas.height = 64
    const context = canvas.getContext('2d', { willReadFrequently: true })
    if (context === null) throw new Error('2D canvas is unavailable')
    const decode = (video: HTMLVideoElement | null): number | null => {
      context.fillStyle = '#000'
      context.fillRect(0, 0, canvas.width, canvas.height)
      if (video === null || video.readyState < HTMLMediaElement.HAVE_CURRENT_DATA) return null
      context.drawImage(video, 0, 0, 320, 240)
      const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data
      const white = (x: number, y: number): boolean => (pixels[(y * canvas.width + x) * 4] ?? 0) > 128
      if (!white(40, 56)) return null
      let index = 0
      let parity = 0
      for (let bit = 0; bit < 16; bit += 1) {
        if (white(40 + bit * 16, 40)) {
          index += 2 ** bit
          parity ^= 1
        }
      }
      return (white(296, 40) ? 1 : 0) === parity ? index : null
    }
    const playing = (cameraId: string): HTMLVideoElement | null => (
      document.querySelector<HTMLVideoElement>(`.camera-window[data-camera="${cameraId}"] video[data-testid="camera-video"]`)
    )
    const samples: { at: number; frames: (number | null)[] }[] = []
    const start = performance.now()
    await new Promise<void>((resolve) => {
      const step = (now: number): void => {
        samples.push({ at: now - start, frames: cameraIds.map((cameraId) => decode(playing(cameraId))) })
        if (now - start < duration) requestAnimationFrame(step)
        else resolve()
      }
      requestAnimationFrame(step)
    })
    return samples
  }, { duration: durationMs, cameraIds: cameras })
}

/** Frames between the most and least advanced camera; null when a window is blank. */
export function frameSpread(sample: FrameSample): number | null {
  const frames = sample.frames.filter((frame): frame is number => frame !== null)
  if (frames.length !== sample.frames.length) return null
  return Math.max(...frames) - Math.min(...frames)
}

export type SpreadSummary = {
  readonly samples: number
  readonly blank: number
  readonly maxSpread: number
  readonly medianSpread: number
  /** Share of samples where every camera shows the same frame. */
  readonly identical: number
}

export function summarizeSpread(samples: readonly FrameSample[]): SpreadSummary {
  const spreads = samples.map(frameSpread)
  const measured = spreads.filter((spread): spread is number => spread !== null).sort((left, right) => left - right)
  return {
    samples: samples.length,
    blank: spreads.length - measured.length,
    maxSpread: measured.at(-1) ?? Number.POSITIVE_INFINITY,
    medianSpread: measured.at(Math.floor(measured.length / 2)) ?? Number.POSITIVE_INFINITY,
    identical: measured.length === 0 ? 0 : measured.filter((spread) => spread === 0).length / measured.length,
  }
}

/** Samples from `fromMs` after the first sample on. */
export function after(samples: readonly FrameSample[], fromMs: number): readonly FrameSample[] {
  return samples.filter((sample) => sample.at >= fromMs)
}

/** Waits until every camera shows an advancing counter (playback has started everywhere). */
export async function waitForAllCamerasPlaying(page: Page): Promise<void> {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    const samples = await sampleCameraFrames(page, 150)
    const first = samples.at(0)
    const last = samples.at(-1)
    if (first !== undefined && last !== undefined && last.frames.every((frame, index) => {
      const before = first.frames[index]
      return frame !== null && before !== null && before !== undefined && frame > before
    })) return
  }
  throw new Error('the six cameras never played together')
}

const metricsPath = '.omo/evidence/teslacam-viewer/json/sync-metrics.json'

/** Records measured numbers for the README performance table (merged into one JSON file). */
export async function recordSyncMetric(name: string, value: unknown): Promise<void> {
  const existing: unknown = await readFile(metricsPath, 'utf8').then((text) => JSON.parse(text), () => ({}))
  const metrics = typeof existing === 'object' && existing !== null ? { ...existing, [name]: value } : { [name]: value }
  await mkdir('.omo/evidence/teslacam-viewer/json', { recursive: true })
  await writeFile(metricsPath, `${JSON.stringify(metrics, null, 2)}\n`)
}
