import { act, renderHook } from '@testing-library/react'
import { vi } from 'vitest'

import type { CameraId, TeslaCamEvent } from '../../teslacam/types'
import { useSynchronizedPlayback } from './use-synchronized-playback'

/** A video at the end of its media, as the browser reports it when `ended` fires. */
function atEnd(element: HTMLVideoElement): HTMLVideoElement {
  Object.defineProperty(element, 'ended', { configurable: true, value: true })
  return element
}

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

function video(): HTMLVideoElement {
  const element = document.createElement('video')
  Object.defineProperty(element, 'readyState', { configurable: true, value: 2 })
  element.play = vi.fn(() => Promise.resolve())
  element.pause = vi.fn()
  element.load = vi.fn()
  return element
}

describe('useSynchronizedPlayback source election', () => {
  it('uses only the elected clock and enters the next segment at zero', () => {
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
      hook.result.current.handleVideoEvent('front', front, 'timeupdate')
    })
    expect(hook.result.current.elapsedSeconds).toBe(20)

    act(() => {
      front.currentTime = 60
      back.currentTime = 59.7
      hook.result.current.handleVideoEvent('front', front, 'timeupdate')
    })
    expect(hook.result.current.segmentIndex).toBe(1)
    expect(hook.result.current.segmentOffsetSeconds).toBe(0)
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
      hook.result.current.handleVideoEvent('front', atEnd(front), 'ended')
    })

    expect(hook.result.current.elapsedSeconds).toBe(60)
    expect(hook.result.current.playing).toBe(false)
  })

  it('re-elects the clock source when the preferred camera changes', () => {
    const front = video()
    const back = video()
    const initialProps: { readonly preferred: CameraId | null } = { preferred: null }
    const hook = renderHook(
      ({ preferred }: { readonly preferred: CameraId | null }) =>
        useSynchronizedPlayback(eventWithSegments(1), preferred),
      { initialProps },
    )

    act(() => {
      hook.result.current.registerVideo('front', front)
      hook.result.current.registerVideo('back', back)
      hook.result.current.handleVideoEvent('front', front, 'ready')
      hook.result.current.handleVideoEvent('back', back, 'ready')
      front.currentTime = 10
      hook.result.current.handleVideoEvent('front', front, 'timeupdate')
    })
    expect(hook.result.current.elapsedSeconds).toBe(10)

    hook.rerender({ preferred: 'back' })
    act(() => {
      front.currentTime = 20
      hook.result.current.handleVideoEvent('front', front, 'timeupdate')
    })
    expect(hook.result.current.elapsedSeconds).toBe(10)

    act(() => {
      back.currentTime = 18
      hook.result.current.handleVideoEvent('back', back, 'timeupdate')
    })
    expect(hook.result.current.elapsedSeconds).toBe(18)
  })
})
