import { act, renderHook } from '@testing-library/react'
import { vi } from 'vitest'

import type { TeslaCamEvent } from '../../teslacam/types'
import { useSynchronizedPlayback } from './use-synchronized-playback'

function eventWithSegments(count: number): TeslaCamEvent {
  return {
    id: 'playback-event',
    category: 'recent',
    timestamp: '2026-08-21_12-00-00',
    segments: Array.from({ length: count }, (_, index) => ({
      timestamp: `2026-08-21_12-${String(index).padStart(2, '0')}-00`,
      cameras: {},
    })),
  }
}

function video(playResult: Promise<void> = Promise.resolve()): HTMLVideoElement {
  const element = document.createElement('video')
  Object.defineProperty(element, 'readyState', { configurable: true, value: 2 })
  element.play = vi.fn(() => playResult)
  element.pause = vi.fn()
  element.load = vi.fn()
  return element
}

function deferred(): {
  readonly promise: Promise<void>
  readonly resolve: () => void
  readonly reject: (reason: DOMException) => void
} {
  let resolve: () => void = () => undefined
  let reject: (reason: DOMException) => void = () => undefined
  const promise = new Promise<void>((settle, fail) => {
    resolve = settle
    reject = fail
  })
  return { promise, resolve, reject }
}

describe('useSynchronizedPlayback', () => {
  it('initializes a paused playback at a clamped non-zero segment position', () => {
    const front = video()
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(2), null, 65))

    expect(hook.result.current.elapsedSeconds).toBe(65)
    expect(hook.result.current.segmentIndex).toBe(1)
    expect(hook.result.current.segmentOffsetSeconds).toBe(5)
    expect(hook.result.current.playing).toBe(false)

    act(() => {
      hook.result.current.registerVideo('front', front)
      hook.result.current.handleVideoEvent('front', front, 'ready')
    })
    expect(front.currentTime).toBe(5)

    act(() => {
      front.currentTime = 9
      hook.result.current.handleVideoEvent('front', front, 'ready')
    })
    expect(front.currentTime).toBe(9)
  })

  it('automatically starts every ready stream and isolates one rejected play call', async () => {
    const rejected = video(Promise.reject(new DOMException('', 'NotAllowedError')))
    const playing = video()
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(2)))

    act(() => {
      hook.result.current.registerVideo('front', rejected)
      hook.result.current.registerVideo('back', playing)
      hook.result.current.handleVideoEvent('front', rejected, 'ready')
      hook.result.current.handleVideoEvent('back', playing, 'ready')
    })
    await act(async () => Promise.resolve())

    expect(rejected.play).toHaveBeenCalledTimes(1)
    expect(playing.play).toHaveBeenCalledTimes(1)
    expect(hook.result.current.playing).toBe(true)
  })

  it('stays paused and announces recovery when every ready stream rejects playback', async () => {
    const front = video(Promise.reject(new DOMException('', 'NotAllowedError')))
    const back = video(Promise.reject(new DOMException('', 'NotAllowedError')))
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(1)))

    act(() => {
      hook.result.current.registerVideo('front', front)
      hook.result.current.registerVideo('back', back)
      hook.result.current.handleVideoEvent('front', front, 'ready')
      hook.result.current.handleVideoEvent('back', back, 'ready')
    })
    await act(async () => hook.result.current.play())

    expect(hook.result.current.playing).toBe(false)
    expect(hook.result.current.message).toMatch(/再試行/)

    act(() => {
      hook.result.current.handleVideoEvent('front', front, 'loading')
      front.play = vi.fn(async () => undefined)
      hook.result.current.handleVideoEvent('front', front, 'ready')
    })
    await act(async () => Promise.resolve())

    expect(front.play).not.toHaveBeenCalled()
  })

  it('does not let stale unregister cleanup remove a replacement element', async () => {
    const stale = video()
    const replacement = video()
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(1)))

    act(() => {
      hook.result.current.registerVideo('front', stale)
      hook.result.current.registerVideo('front', replacement)
      hook.result.current.unregisterVideo('front', stale)
      hook.result.current.handleVideoEvent('front', replacement, 'ready')
    })
    await act(async () => Promise.resolve())

    expect(stale.play).not.toHaveBeenCalled()
    expect(replacement.play).toHaveBeenCalledTimes(1)
  })

  it('synchronizes explicit seek, skip, rate, and pause commands immediately', () => {
    const front = video()
    const back = video()
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(2)))

    act(() => {
      hook.result.current.registerVideo('front', front)
      hook.result.current.registerVideo('back', back)
      hook.result.current.handleVideoEvent('front', front, 'ready')
      hook.result.current.handleVideoEvent('back', back, 'ready')
      hook.result.current.seek(45)
      hook.result.current.skip(10)
      hook.result.current.setRate(1.5)
      hook.result.current.pause()
    })

    expect(hook.result.current.elapsedSeconds).toBe(55)
    expect(front.currentTime).toBe(55)
    expect(back.currentTime).toBe(55)
    expect(front.playbackRate).toBe(1.5)
    expect(back.playbackRate).toBe(1.5)
    expect(front.pause).toHaveBeenCalled()
    expect(back.pause).toHaveBeenCalled()
  })

  it('preserves pending play intent and physical playback after seek', async () => {
    const pending = deferred()
    const front = video(pending.promise)
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(2)))

    act(() => {
      hook.result.current.registerVideo('front', front)
      hook.result.current.handleVideoEvent('front', front, 'ready')
    })
    const playPromise = hook.result.current.play()
    act(() => hook.result.current.seek(20))
    pending.resolve()
    await act(async () => playPromise)

    expect(front.currentTime).toBe(20)
    expect(hook.result.current.playing).toBe(true)
  })

  it('preserves pending play intent and physical playback after skip', async () => {
    const pending = deferred()
    const front = video(pending.promise)
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(2)))

    act(() => {
      hook.result.current.registerVideo('front', front)
      hook.result.current.handleVideoEvent('front', front, 'ready')
    })
    const playPromise = hook.result.current.play()
    act(() => hook.result.current.skip(10))
    pending.resolve()
    await act(async () => playPromise)

    expect(front.currentTime).toBe(10)
    expect(hook.result.current.playing).toBe(true)
  })

  it('preserves pending play intent and physical playback after a rate change', async () => {
    const pending = deferred()
    const front = video(pending.promise)
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(2)))

    act(() => {
      hook.result.current.registerVideo('front', front)
      hook.result.current.handleVideoEvent('front', front, 'ready')
    })
    const playPromise = hook.result.current.play()
    act(() => hook.result.current.setRate(1.5))
    pending.resolve()
    await act(async () => playPromise)

    expect(front.playbackRate).toBe(1.5)
    expect(hook.result.current.playing).toBe(true)
  })

  it('ignores old-source settlement after a segment-changing seek while preserving intent', async () => {
    const pending = deferred()
    const front = video(pending.promise)
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(2)))

    act(() => {
      hook.result.current.registerVideo('front', front)
      hook.result.current.handleVideoEvent('front', front, 'ready')
    })
    const oldPlay = hook.result.current.play()
    act(() => {
      hook.result.current.seek(60)
    })
    expect(front.pause).toHaveBeenCalledTimes(1)
    act(() => {
      hook.result.current.handleVideoEvent('front', front, 'loading')
      front.play = vi.fn(async () => undefined)
      hook.result.current.handleVideoEvent('front', front, 'ready')
    })
    await act(async () => Promise.resolve())
    pending.resolve()
    await act(async () => oldPlay)

    expect(front.play).toHaveBeenCalledTimes(1)
    expect(hook.result.current.segmentIndex).toBe(1)
    expect(hook.result.current.playing).toBe(true)
  })

})
