import { describe, expect, it, vi } from 'vitest'

import type { CameraId } from '../../teslacam/types'
import { CompositeVideoExportError } from './composite-recorder'
import {
  renderCompositeSegment,
  type CompositeRenderClip,
  type CompositeRenderRuntime,
} from './composite-renderer'
import type { CompositeFrameSource } from './composite-frame'

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
      drawFrame: () => undefined,
      drawVideo: () => undefined,
      segmentIndex: 0,
      segmentCount: 1,
      recorder: lifecycle,
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
    const frames: (readonly CompositeFrameSource[])[] = []
    const progress = vi.fn()
    const rendered = renderCompositeSegment({
      clips: [clip(1)],
      drawFrame: (sources) => frames.push(sources),
      drawVideo: () => undefined,
      segmentIndex: 0,
      segmentCount: 1,
      recorder: recorder(),
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

    expect(frames.length).toBeGreaterThan(20)
    expect(frames.length).toBeLessThanOrEqual(31)
    expect(progress.mock.calls.length).toBeLessThanOrEqual(5)
    expect(progress).toHaveBeenLastCalledWith(1)
  })

  it('hands each playable video to the frame renderer, which draws it at the fitted rectangle', async () => {
    const animation = animationDriver()
    const source = clip(0.2)
    const drawVideo = vi.fn()
    let frame: readonly CompositeFrameSource[] = []
    const rendered = renderCompositeSegment({
      clips: [source],
      drawFrame: (sources) => {
        frame = sources
      },
      drawVideo,
      segmentIndex: 0,
      segmentCount: 1,
      recorder: recorder(),
      runtime: {
        ...animation.runtime,
        now: () => 0,
        isPageVisible: () => true,
        subscribePageVisibility: () => () => undefined,
      },
    })
    await animation.waitUntilScheduled()

    animation.step(0)
    expect(frame.map(({ cameraId, width, height }) => [cameraId, width, height])).toEqual([['front', 1920, 1080]])
    frame[0]?.draw({ x: 464, y: 45, width: 352, height: 198 })
    expect(drawVideo).toHaveBeenCalledWith(source.video, 464, 45, 352, 198)
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
      drawFrame: () => undefined,
      drawVideo: () => undefined,
      segmentIndex: 0,
      segmentCount: 1,
      recorder: lifecycle,
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
