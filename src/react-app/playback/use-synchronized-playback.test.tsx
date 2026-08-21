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

describe('useSynchronizedPlayback', () => {
  it('starts every ready stream before settling and isolates one rejected play call', async () => {
    const rejected = video(Promise.reject(new DOMException('', 'NotAllowedError')))
    const playing = video()
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(2)))

    act(() => {
      hook.result.current.registerVideo('front', rejected)
      hook.result.current.registerVideo('back', playing)
      hook.result.current.handleVideoEvent('front', rejected, 'ready')
      hook.result.current.handleVideoEvent('back', playing, 'ready')
    })
    await act(async () => hook.result.current.play())

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
    await act(async () => hook.result.current.play())

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

  it('uses only the elected clock, corrects drift beyond but not at 250ms, and enters the next segment at zero', () => {
    const front = video()
    const back = video()
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(2)))

    act(() => {
      hook.result.current.registerVideo('front', front)
      hook.result.current.registerVideo('back', back)
      hook.result.current.handleVideoEvent('front', front, 'ready')
      hook.result.current.handleVideoEvent('back', back, 'ready')
      back.currentTime = 20
      hook.result.current.handleVideoEvent('back', back, 'timeupdate')
    })
    expect(hook.result.current.elapsedSeconds).toBe(0)

    act(() => {
      front.currentTime = 20
      back.currentTime = 19.75
      hook.result.current.handleVideoEvent('front', front, 'timeupdate')
    })
    expect(back.currentTime).toBe(19.75)

    act(() => {
      front.currentTime = 60
      back.currentTime = 59.7
      hook.result.current.handleVideoEvent('front', front, 'timeupdate')
    })
    expect(hook.result.current.segmentIndex).toBe(1)
    expect(hook.result.current.segmentOffsetSeconds).toBe(0)
  })

  it('ignores stale play settlement after pause', async () => {
    let settle: () => void = () => undefined
    const pending = new Promise<void>((resolve) => {
      settle = resolve
    })
    const front = video(pending)
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(1)))

    act(() => {
      hook.result.current.registerVideo('front', front)
      hook.result.current.handleVideoEvent('front', front, 'ready')
    })
    const playPromise = hook.result.current.play()
    act(() => hook.result.current.pause())
    settle()
    await act(async () => playPromise)

    expect(hook.result.current.playing).toBe(false)
  })

  it('re-elects the source when the preferred stream fails and pauses at the final ended event', () => {
    const front = video()
    const back = video()
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(1), 'back'))

    act(() => {
      hook.result.current.registerVideo('front', front)
      hook.result.current.registerVideo('back', back)
    })
    act(() => {
      hook.result.current.handleVideoEvent('front', front, 'ready')
      hook.result.current.handleVideoEvent('back', back, 'ready')
      hook.result.current.handleVideoEvent('back', back, 'unavailable')
      front.currentTime = 60
      hook.result.current.handleVideoEvent('front', front, 'ended')
    })

    expect(hook.result.current.elapsedSeconds).toBe(60)
    expect(hook.result.current.playing).toBe(false)
  })
})
