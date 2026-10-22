import { createFrameSyncLoop } from './frame-sync-loop'
import { createPlaybackRuntime, type PlaybackRuntime } from './playback-runtime'

type FrameCallback = (now: number, metadata: VideoFrameCallbackMetadata) => void

type FakeVideo = {
  readonly element: HTMLVideoElement
  /** Puts a frame of `mediaTime` on screen at `displayedAt` and runs the pending frame callback. */
  readonly present: (mediaTime: number, displayedAt: number) => void
  readonly pending: () => number
}

function fakeVideo(currentTime: number): FakeVideo {
  const element = document.createElement('video')
  const callbacks = new Map<number, FrameCallback>()
  let nextHandle = 1
  Object.defineProperty(element, 'paused', { configurable: true, writable: true, value: false })
  Object.defineProperty(element, 'readyState', { configurable: true, value: HTMLMediaElement.HAVE_ENOUGH_DATA })
  element.currentTime = currentTime
  element.requestVideoFrameCallback = (callback: FrameCallback): number => {
    const handle = nextHandle
    nextHandle += 1
    callbacks.set(handle, callback)
    return handle
  }
  element.cancelVideoFrameCallback = (handle: number): void => {
    callbacks.delete(handle)
  }
  return {
    element,
    present: (mediaTime, displayedAt) => {
      const due = [...callbacks.values()]
      callbacks.clear()
      const metadata: VideoFrameCallbackMetadata = {
        mediaTime, expectedDisplayTime: displayedAt, presentationTime: displayedAt, presentedFrames: 1, width: 320, height: 240,
      }
      for (const callback of due) callback(displayedAt, metadata)
    },
    pending: () => callbacks.size,
  }
}

function runtimeWith(videos: Readonly<Record<string, FakeVideo>>, source: 'front' | 'back'): PlaybackRuntime {
  const runtime = createPlaybackRuntime()
  for (const cameraId of ['front', 'back'] as const) {
    const video = videos[cameraId]
    if (video === undefined) continue
    runtime.registrations.set(cameraId, { element: video.element })
    runtime.readyCameras.add(cameraId)
  }
  runtime.sourceCamera = source
  return runtime
}

describe('createFrameSyncLoop', () => {
  it('slows a follower whose clock runs ahead of the source on each frame the source presents', () => {
    const front = fakeVideo(10)
    const back = fakeVideo(10.03)
    const runtime = runtimeWith({ front, back }, 'front')
    const loop = createFrameSyncLoop(runtime)
    loop.watch(front.element)
    loop.watch(back.element)

    front.present(10, 1_000)

    expect(back.element.playbackRate).toBe(1 - 0.12)
    expect(back.element.currentTime).toBe(10.03)
    expect(front.element.playbackRate).toBe(1)
  })

  it('holds a follower whose frames reach the screen a frame early one frame behind on the clock', () => {
    const front = fakeVideo(10)
    const back = fakeVideo(10)
    const runtime = runtimeWith({ front, back }, 'front')
    const loop = createFrameSyncLoop(runtime)
    loop.watch(front.element)
    loop.watch(back.element)

    for (let frame = 0; frame < 30; frame += 1) {
      const at = 1_000 + frame * 16.7
      back.present(10 + 1 / 36, at)
      front.present(10, at)
    }

    expect(back.element.playbackRate).toBe(1 - 0.11)
  })

  it('seeks a follower whose clock is far from the source', () => {
    const front = fakeVideo(10)
    const back = fakeVideo(9.5)
    const runtime = runtimeWith({ front, back }, 'front')
    const loop = createFrameSyncLoop(runtime)
    loop.watch(front.element)
    loop.watch(back.element)

    front.present(10, 1_000)

    expect(back.element.currentTime).toBe(10)
  })

  it('keeps watching every frame and leaves followers alone on their own frames', () => {
    const front = fakeVideo(10)
    const back = fakeVideo(9.5)
    const runtime = runtimeWith({ front, back }, 'front')
    const loop = createFrameSyncLoop(runtime)
    loop.watch(front.element)
    loop.watch(back.element)

    back.present(9.5, 1_000)

    expect(back.element.currentTime).toBe(9.5)
    expect(front.pending()).toBe(1)
    expect(back.pending()).toBe(1)
  })

  it('skips followers that are paused or seeking', () => {
    const front = fakeVideo(10)
    const back = fakeVideo(9)
    const runtime = runtimeWith({ front, back }, 'front')
    const loop = createFrameSyncLoop(runtime)
    loop.watch(front.element)
    loop.watch(back.element)

    Object.defineProperty(back.element, 'paused', { value: true })
    front.present(10, 1_000)
    expect(back.element.currentTime).toBe(9)

    Object.defineProperty(back.element, 'paused', { value: false })
    Object.defineProperty(back.element, 'seeking', { configurable: true, value: true })
    front.present(10.0167, 1_016.7)
    expect(back.element.currentTime).toBe(9)
  })

  it('stops watching an element and restores the chosen rate on the source after re-election', () => {
    const front = fakeVideo(10)
    const back = fakeVideo(10)
    const runtime = runtimeWith({ front, back }, 'front')
    const loop = createFrameSyncLoop(runtime)
    loop.watch(front.element)
    loop.watch(back.element)

    back.element.playbackRate = 0.9
    runtime.sourceCamera = 'back'
    back.present(10, 1_000)
    expect(back.element.playbackRate).toBe(1)

    loop.unwatch(front.element)
    expect(front.pending()).toBe(0)
  })

  it('restores the chosen rate on every camera on reset', () => {
    const front = fakeVideo(10)
    const back = fakeVideo(10.03)
    const runtime = runtimeWith({ front, back }, 'front')
    const loop = createFrameSyncLoop(runtime)
    loop.watch(front.element)
    loop.watch(back.element)
    front.present(10, 1_000)

    runtime.rate = 2
    loop.reset()

    expect(back.element.playbackRate).toBe(2)
    expect(front.element.playbackRate).toBe(2)
  })
})
