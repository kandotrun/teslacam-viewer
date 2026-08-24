import { act, renderHook } from '@testing-library/react'
import { vi } from 'vitest'

import type { TeslaCamEvent } from '../../teslacam/types'
import { useSynchronizedPlayback } from './use-synchronized-playback'

function eventWithSegments(count: number): TeslaCamEvent {
  return {
    id: 'autoplay-event',
    category: 'sentry',
    timestamp: '2026-08-21_12-00-00',
    segments: Array.from({ length: count }, (_, index) => ({
      timestamp: `2026-08-21_12-${String(index).padStart(2, '0')}-00`,
      cameras: {},
    })),
  }
}

function video(playResult: Promise<void> = Promise.resolve()): HTMLVideoElement {
  const element = document.createElement('video')
  element.play = vi.fn(() => playResult)
  element.pause = vi.fn()
  return element
}

describe('useSynchronizedPlayback autoplay', () => {
  it('autoplays a newly ready stream once at the selected initial offset', async () => {
    const front = video()
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(2), null, 65))

    act(() => hook.result.current.registerVideo('front', front))
    expect(front.play).not.toHaveBeenCalled()

    act(() => hook.result.current.handleVideoEvent('front', front, 'ready'))
    await act(async () => Promise.resolve())

    expect(front.currentTime).toBe(5)
    expect(front.play).toHaveBeenCalledTimes(1)
    expect(hook.result.current.playing).toBe(true)
  })

  it('does not replay or rewind an already ready stream', async () => {
    const front = video()
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(2), null, 65))

    act(() => hook.result.current.registerVideo('front', front))
    act(() => hook.result.current.handleVideoEvent('front', front, 'ready'))
    await act(async () => Promise.resolve())
    front.currentTime = 9

    act(() => hook.result.current.handleVideoEvent('front', front, 'ready'))

    expect(front.currentTime).toBe(9)
    expect(front.play).toHaveBeenCalledTimes(1)
  })

  it('cancels pending autoplay when explicitly paused before readiness', () => {
    const front = video()
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(1)))

    act(() => {
      hook.result.current.registerVideo('front', front)
      hook.result.current.pause()
      hook.result.current.handleVideoEvent('front', front, 'ready')
    })

    expect(front.play).not.toHaveBeenCalled()
    expect(hook.result.current.playing).toBe(false)
  })

  it('keeps the retry recovery message when automatic playback is rejected', async () => {
    const rejected = Promise.reject(new DOMException('', 'NotAllowedError'))
    void rejected.catch(() => undefined)
    const front = video(rejected)
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(1)))

    act(() => hook.result.current.registerVideo('front', front))
    act(() => hook.result.current.handleVideoEvent('front', front, 'ready'))
    await act(async () => Promise.resolve())

    expect(front.play).toHaveBeenCalledTimes(1)
    expect(hook.result.current.playing).toBe(false)
    expect(hook.result.current.message).toMatch(/再試行/)
  })
})
