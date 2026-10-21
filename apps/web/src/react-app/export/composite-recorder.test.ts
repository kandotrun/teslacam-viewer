import { describe, expect, it } from 'vitest'

import {
  CompositeRecorderLifecycle,
  type CompositeRecorder,
} from './composite-recorder'

type RecorderHarness = {
  readonly recorder: CompositeRecorder
  readonly fail: () => void
}

function recorderHarness(initialState: RecordingState): RecorderHarness {
  let state = initialState
  const recorder: CompositeRecorder = {
    mimeType: 'video/webm',
    get state() { return state },
    ondataavailable: null,
    onerror: null,
    onpause: null,
    onresume: null,
    onstop: null,
    start: () => { state = 'recording' },
    pause: () => undefined,
    resume: () => undefined,
    stop: () => { state = 'inactive' },
  }
  return {
    recorder,
    fail: () => recorder.onerror?.(new ErrorEvent('error')),
  }
}

describe('CompositeRecorderLifecycle', () => {
  it('settles pause when export is aborted', async () => {
    const controller = new AbortController()
    const lifecycle = new CompositeRecorderLifecycle(recorderHarness('recording').recorder, controller.signal)
    const paused = lifecycle.pause()

    controller.abort()

    await expect(paused).rejects.toMatchObject({ code: 'aborted' })
  })

  it('settles pause when MediaRecorder fails', async () => {
    const harness = recorderHarness('recording')
    const lifecycle = new CompositeRecorderLifecycle(harness.recorder)
    const paused = lifecycle.pause()

    harness.fail()

    await expect(paused).rejects.toMatchObject({ code: 'recording-failed' })
  })

  it('settles resume when export is aborted', async () => {
    const controller = new AbortController()
    const lifecycle = new CompositeRecorderLifecycle(recorderHarness('paused').recorder, controller.signal)
    const resumed = lifecycle.resume()

    controller.abort()

    await expect(resumed).rejects.toMatchObject({ code: 'aborted' })
  })

  it('settles resume when MediaRecorder fails', async () => {
    const harness = recorderHarness('paused')
    const lifecycle = new CompositeRecorderLifecycle(harness.recorder)
    const resumed = lifecycle.resume()

    harness.fail()

    await expect(resumed).rejects.toMatchObject({ code: 'recording-failed' })
  })

  it('does not mask the original export failure when forced recorder stop throws', () => {
    const harness = recorderHarness('recording')
    harness.recorder.stop = () => { throw new DOMException('stop failed', 'InvalidStateError') }
    const lifecycle = new CompositeRecorderLifecycle(harness.recorder)

    expect(() => lifecycle.forceStop()).not.toThrow()
  })
})
