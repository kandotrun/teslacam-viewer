import { act, renderHook } from '@testing-library/react'
import { vi } from 'vitest'

import type { TeslaCamEvent } from '../../teslacam/types'
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

type FrameCallback = (now: number, metadata: VideoFrameCallbackMetadata) => void

type PlayingVideo = {
  readonly element: HTMLVideoElement
  readonly present: (mediaTime: number, displayedAt: number) => void
  readonly seeks: () => readonly number[]
}

/** A playing video that reports presented frames and records every seek. */
function playingVideo(currentTime = 0): PlayingVideo {
  const element = document.createElement('video')
  const callbacks = new Map<number, FrameCallback>()
  const seeks: number[] = []
  let time = currentTime
  let nextHandle = 1
  Object.defineProperty(element, 'readyState', { configurable: true, value: HTMLMediaElement.HAVE_ENOUGH_DATA })
  Object.defineProperty(element, 'paused', { configurable: true, value: false })
  Object.defineProperty(element, 'currentTime', {
    configurable: true,
    get: () => time,
    set: (next: number) => {
      seeks.push(next)
      time = next
    },
  })
  element.play = vi.fn(async () => undefined)
  element.pause = vi.fn()
  element.load = vi.fn()
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
      time = mediaTime
      const due = [...callbacks.values()]
      callbacks.clear()
      for (const callback of due) {
        callback(displayedAt, {
          mediaTime, expectedDisplayTime: displayedAt, presentationTime: displayedAt, presentedFrames: 1, width: 320, height: 240,
        })
      }
    },
    seeks: () => seeks,
  }
}

function presentTogether(source: PlayingVideo, follower: PlayingVideo, followerLead: number, frames: number): void {
  for (let frame = 0; frame < frames; frame += 1) {
    const at = 1_000 + frame * 16.7
    follower.present(5 + followerLead + frame * 0.0167, at)
    source.present(5 + frame * 0.0167, at)
  }
}

describe('useSynchronizedPlayback frame sync', () => {
  it('nudges a follower towards the source on the frames the source presents, without seeking it', () => {
    const front = playingVideo()
    const back = playingVideo()
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(1)))
    act(() => {
      hook.result.current.registerVideo('front', front.element)
      hook.result.current.registerVideo('back', back.element)
      hook.result.current.handleVideoEvent('front', front.element, 'ready')
      hook.result.current.handleVideoEvent('back', back.element, 'ready')
    })

    presentTogether(front, back, 0.05, 30)

    expect(back.element.playbackRate).toBeLessThan(1)
    expect(back.seeks()).toEqual([])
    expect(front.element.playbackRate).toBe(1)
  })

  it('no longer seeks followers from timeupdate', () => {
    const front = playingVideo()
    const back = playingVideo()
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(1)))
    act(() => {
      hook.result.current.registerVideo('front', front.element)
      hook.result.current.registerVideo('back', back.element)
      hook.result.current.handleVideoEvent('front', front.element, 'ready')
      hook.result.current.handleVideoEvent('back', back.element, 'ready')
    })

    act(() => {
      front.element.currentTime = 20
      back.element.currentTime = 19
      hook.result.current.handleVideoEvent('front', front.element, 'timeupdate')
    })

    expect(back.element.currentTime).toBe(19)
    expect(hook.result.current.elapsedSeconds).toBe(20)
  })

  it('restores the chosen rate on followers when playback pauses', () => {
    const front = playingVideo()
    const back = playingVideo()
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(1)))
    act(() => {
      hook.result.current.registerVideo('front', front.element)
      hook.result.current.registerVideo('back', back.element)
      hook.result.current.handleVideoEvent('front', front.element, 'ready')
      hook.result.current.handleVideoEvent('back', back.element, 'ready')
    })
    presentTogether(front, back, 0.05, 30)

    act(() => hook.result.current.pause())

    expect(back.element.playbackRate).toBe(1)
  })

  it('leaves the old segment where it is when the source ends and the next segment starts', async () => {
    const front = playingVideo(59.9)
    const back = playingVideo(59.9)
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(2), null, 59.9))
    act(() => {
      hook.result.current.registerVideo('front', front.element)
      hook.result.current.registerVideo('back', back.element)
      hook.result.current.handleVideoEvent('front', front.element, 'ready')
      hook.result.current.handleVideoEvent('back', back.element, 'ready')
    })
    await act(async () => Promise.resolve())

    act(() => hook.result.current.handleVideoEvent('front', atEnd(front.element), 'ended'))

    expect(hook.result.current.segmentIndex).toBe(1)
    expect(front.seeks()).not.toContain(0)
    expect(back.seeks()).not.toContain(0)
  })

  it('plays preloaded next-segment videos swapped in for each camera, without seeking them', async () => {
    const front = playingVideo(59.9)
    const back = playingVideo(59.9)
    const nextFront = playingVideo(0)
    const nextBack = playingVideo(0)
    const hook = renderHook(() => useSynchronizedPlayback(eventWithSegments(2), null, 59.9))
    act(() => {
      hook.result.current.registerVideo('front', front.element)
      hook.result.current.registerVideo('back', back.element)
      hook.result.current.handleVideoEvent('front', front.element, 'ready')
      hook.result.current.handleVideoEvent('back', back.element, 'ready')
    })
    await act(async () => Promise.resolve())
    expect(hook.result.current.playing).toBe(true)

    act(() => {
      hook.result.current.handleVideoEvent('front', atEnd(front.element), 'ended')
      hook.result.current.registerVideo('front', nextFront.element)
      hook.result.current.handleVideoEvent('front', nextFront.element, 'ready')
      hook.result.current.registerVideo('back', nextBack.element)
      hook.result.current.handleVideoEvent('back', nextBack.element, 'ready')
    })
    await act(async () => Promise.resolve())

    expect(nextFront.element.play).toHaveBeenCalledTimes(1)
    expect(nextBack.element.play).toHaveBeenCalledTimes(1)
    expect(nextFront.seeks()).toEqual([])
    expect(nextBack.seeks()).toEqual([])
    expect(hook.result.current.playing).toBe(true)
    expect(hook.result.current.message).toBeNull()
  })
})
