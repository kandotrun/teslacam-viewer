import { describe, expect, it, vi } from 'vitest'

import type { CameraId } from '../../teslacam/types'
import { CompositeVideoExportError } from './composite-recorder'
import {
  renderCompositeSegment,
  type CompositeRenderClip,
  type CompositeRenderRuntime,
} from './composite-renderer'
import type { CompositeCanvasVisualTokens } from './composite-visual-tokens'

const visualTokens: CompositeCanvasVisualTokens = {
  canvas: 'rgb(0, 0, 0)',
  cameraSurface: 'rgb(0, 0, 0)',
  labelScrim: 'rgb(0, 0, 0, 0.72)',
  labelText: 'rgb(255, 255, 255)',
  labelFont: '500 13px -apple-system, BlinkMacSystemFont, "Hiragino Sans", sans-serif',
  labelInset: 8,
  labelPaddingInline: 8,
  labelHeight: 24,
  labelBaseline: 17,
}

type AnimationDriver = {
  readonly runtime: Pick<CompositeRenderRuntime, 'requestAnimationFrame' | 'cancelAnimationFrame'>
  readonly step: (timestamp: number) => void
  readonly waitUntilScheduled: () => Promise<void>
}

function animationDriver(): AnimationDriver {
  let nextHandle = 1
  const callbacks = new Map<number, FrameRequestCallback>()
  let scheduled: (() => void) | undefined
  return {
    runtime: {
      requestAnimationFrame: (callback) => {
        const handle = nextHandle
        nextHandle += 1
        callbacks.set(handle, callback)
        scheduled?.()
        scheduled = undefined
        return handle
      },
      cancelAnimationFrame: (handle) => { callbacks.delete(handle) },
    },
    step: (timestamp) => {
      const pending = [...callbacks.values()]
      callbacks.clear()
      for (const callback of pending) callback(timestamp)
    },
    waitUntilScheduled: () => callbacks.size > 0 ? Promise.resolve() : new Promise((resolve) => { scheduled = resolve }),
  }
}

function clip(duration: number): CompositeRenderClip {
  const video = document.createElement('video')
  Object.defineProperties(video, {
    duration: { value: duration },
    videoWidth: { value: 1920 },
    videoHeight: { value: 1080 },
    readyState: { value: HTMLMediaElement.HAVE_CURRENT_DATA },
    play: { value: vi.fn(async () => undefined) },
    pause: { value: vi.fn() },
  })
  return { cameraId: 'front' satisfies CameraId, video, duration }
}

function context(drawImage: () => void): CanvasRenderingContext2D {
  const canvas = document.createElement('canvas')
  Object.defineProperty(canvas, 'getContext', {
    value: () => ({
      drawImage,
      fillRect: () => undefined,
      fillText: () => undefined,
      measureText: () => ({ width: 80 }),
    }),
  })
  const value = canvas.getContext('2d')
  if (value === null) throw new Error('canvas context is missing')
  return value
}

function recorder() {
  return {
    pause: vi.fn(async () => undefined),
    resume: vi.fn(async () => undefined),
    subscribeFailure: () => () => undefined,
  }
}

describe('renderCompositeSegment', () => {
  it('does not advance logical time across a hidden visibility gap', async () => {
    const animation = animationDriver()
    const source = clip(0.2)
    const lifecycle = recorder()
    let visible = true
    let visibilityListener = (nextVisible: boolean) => { visible = nextVisible }
    let completed = false
    const rendered = renderCompositeSegment({
      clips: [source],
      context: context(() => undefined),
      vehicleImage: document.createElement('img'),
      segmentIndex: 0,
      segmentCount: 1,
      recorder: lifecycle,
      visualTokens,
      runtime: {
        ...animation.runtime,
        now: () => 0,
        isPageVisible: () => visible,
        subscribePageVisibility: (listener) => {
          visibilityListener = listener
          return () => undefined
        },
      },
    }).then(() => { completed = true })
    await animation.waitUntilScheduled()

    animation.step(0)
    animation.step(50)
    visible = false
    visibilityListener(false)
    await Promise.resolve()
    animation.step(10_000)
    visible = true
    visibilityListener(true)
    await animation.waitUntilScheduled()
    animation.step(10_000)
    animation.step(10_050)

    expect(completed).toBe(false)
    animation.step(10_150)
    await rendered
    expect(lifecycle.pause).toHaveBeenCalledTimes(1)
    expect(lifecycle.resume).toHaveBeenCalledTimes(1)
    expect(source.video.pause).toHaveBeenCalled()
    expect(source.video.play).toHaveBeenCalledTimes(2)
  })

  it('caps rendering at 30fps and progress at four updates per second', async () => {
    const animation = animationDriver()
    const drawImage = vi.fn()
    const progress = vi.fn()
    const vehicleImage = document.createElement('img')
    const rendered = renderCompositeSegment({
      clips: [clip(1)],
      context: context(drawImage),
      vehicleImage,
      segmentIndex: 0,
      segmentCount: 1,
      recorder: recorder(),
      visualTokens,
      runtime: {
        ...animation.runtime,
        now: () => 0,
        isPageVisible: () => true,
        subscribePageVisibility: () => () => undefined,
      },
      onProgress: progress,
    })
    await animation.waitUntilScheduled()

    for (let frame = 0; frame <= 60; frame += 1) animation.step(frame * 1000 / 60)
    await rendered

    const vehicleDraws = drawImage.mock.calls.filter(([source]) => source === vehicleImage)
    expect(vehicleDraws.length).toBeGreaterThan(20)
    expect(vehicleDraws.length).toBeLessThanOrEqual(31)
    expect(progress.mock.calls.length).toBeLessThanOrEqual(5)
    expect(progress).toHaveBeenLastCalledWith(1)
  })

  it('passes the vehicle image through to the contained center draw', async () => {
    const animation = animationDriver()
    const drawImage = vi.fn()
    const vehicleImage = document.createElement('img')
    Object.defineProperties(vehicleImage, {
      naturalWidth: { value: 360 },
      naturalHeight: { value: 720 },
    })
    const rendered = renderCompositeSegment({
      clips: [clip(0.2)],
      context: context(drawImage),
      vehicleImage,
      segmentIndex: 0,
      segmentCount: 1,
      recorder: recorder(),
      visualTokens,
      runtime: {
        ...animation.runtime,
        now: () => 0,
        isPageVisible: () => true,
        subscribePageVisibility: () => () => undefined,
      },
    })
    await animation.waitUntilScheduled()

    animation.step(0)

    expect(drawImage).toHaveBeenCalledWith(vehicleImage, 594, 260, 92, 184)
    animation.step(200)
    await rendered
  })

  it('settles when the recorder lifecycle fails while hidden', async () => {
    const animation = animationDriver()
    let visibilityListener: (visible: boolean) => void = () => undefined
    let failureListener: (error: CompositeVideoExportError) => void = () => undefined
    const lifecycle = {
      pause: vi.fn(() => new Promise<void>(() => undefined)),
      resume: vi.fn(async () => undefined),
      subscribeFailure: (listener: (error: CompositeVideoExportError) => void) => {
        failureListener = listener
        return () => undefined
      },
    }
    const rendered = renderCompositeSegment({
      clips: [clip(1)],
      context: context(() => undefined),
      vehicleImage: document.createElement('img'),
      segmentIndex: 0,
      segmentCount: 1,
      recorder: lifecycle,
      visualTokens,
      runtime: {
        ...animation.runtime,
        now: () => 0,
        isPageVisible: () => true,
        subscribePageVisibility: (listener) => {
          visibilityListener = listener
          return () => undefined
        },
      },
    })
    await animation.waitUntilScheduled()

    visibilityListener(false)
    failureListener(new CompositeVideoExportError('recording-failed', '動画の録画に失敗しました。'))

    await expect(rendered).rejects.toMatchObject({ code: 'recording-failed' })
  })
})
