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

describe('useSynchronizedPlayback transitions', () => {
  it('keeps a ready stream at its current position when canplay repeats', () => {
    const front = video()
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(1)))

    act(() => {
      hook.result.current.registerVideo('front', front)
      hook.result.current.handleVideoEvent('front', front, 'ready')
      front.currentTime = 1.25
      hook.result.current.handleVideoEvent('front', front, 'ready')
    })

    expect(front.currentTime).toBe(1.25)
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

  it('tracks successful automatic playback after an internal segment transition', async () => {
    const front = video()
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(2)))

    act(() => {
      hook.result.current.registerVideo('front', front)
      hook.result.current.handleVideoEvent('front', front, 'ready')
    })
    await act(async () => Promise.resolve())
    act(() => {
      hook.result.current.handleVideoEvent('front', front, 'ended')
      hook.result.current.handleVideoEvent('front', front, 'loading')
      hook.result.current.handleVideoEvent('front', front, 'ready')
    })
    await act(async () => Promise.resolve())

    expect(front.play).toHaveBeenCalledTimes(2)
    expect(hook.result.current.segmentIndex).toBe(1)
    expect(hook.result.current.playing).toBe(true)
  })

  it('pauses and announces recovery when every new-segment automatic play rejects', async () => {
    const front = video()
    const back = video()
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(2)))

    act(() => {
      hook.result.current.registerVideo('front', front)
      hook.result.current.registerVideo('back', back)
      hook.result.current.handleVideoEvent('front', front, 'ready')
      hook.result.current.handleVideoEvent('back', back, 'ready')
    })
    await act(async () => hook.result.current.play())
    act(() => {
      hook.result.current.handleVideoEvent('front', front, 'ended')
      hook.result.current.handleVideoEvent('front', front, 'loading')
      hook.result.current.handleVideoEvent('back', back, 'loading')
      front.play = vi.fn(async () => Promise.reject(new DOMException('', 'NotAllowedError')))
      back.play = vi.fn(async () => Promise.reject(new DOMException('', 'NotAllowedError')))
      hook.result.current.handleVideoEvent('front', front, 'ready')
      hook.result.current.handleVideoEvent('back', back, 'ready')
    })
    await act(async () => Promise.resolve())

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

  it('keeps automatic playback active when one new segment rejects and another succeeds', async () => {
    const front = video()
    const back = video()
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(2)))

    act(() => {
      hook.result.current.registerVideo('front', front)
      hook.result.current.registerVideo('back', back)
      hook.result.current.handleVideoEvent('front', front, 'ready')
      hook.result.current.handleVideoEvent('back', back, 'ready')
    })
    await act(async () => hook.result.current.play())
    act(() => {
      hook.result.current.handleVideoEvent('front', front, 'ended')
      hook.result.current.handleVideoEvent('front', front, 'loading')
      hook.result.current.handleVideoEvent('back', back, 'loading')
      front.play = vi.fn(async () => Promise.reject(new DOMException('', 'NotAllowedError')))
      back.play = vi.fn(async () => undefined)
      hook.result.current.handleVideoEvent('front', front, 'ready')
      hook.result.current.handleVideoEvent('back', back, 'ready')
    })
    await act(async () => Promise.resolve())

    expect(hook.result.current.playing).toBe(true)
  })

  it('pauses with recovery when every registered stream becomes terminal during intended playback', async () => {
    const front = video()
    const back = video()
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(2)))

    act(() => {
      hook.result.current.registerVideo('front', front)
      hook.result.current.registerVideo('back', back)
      hook.result.current.handleVideoEvent('front', front, 'ready')
      hook.result.current.handleVideoEvent('back', back, 'ready')
    })
    await act(async () => hook.result.current.play())
    act(() => {
      hook.result.current.handleVideoEvent('front', front, 'loading')
      hook.result.current.handleVideoEvent('back', back, 'loading')
      hook.result.current.handleVideoEvent('front', front, 'unavailable')
      hook.result.current.handleVideoEvent('back', back, 'unavailable')
    })

    expect(hook.result.current.playing).toBe(false)
    expect(hook.result.current.message).toMatch(/再試行/)
  })
})
