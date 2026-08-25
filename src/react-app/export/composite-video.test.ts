import { describe, expect, it, vi } from 'vitest'

import type { LocalFileRef, TeslaCamEvent } from '../../teslacam/types'
import {
  CompositeVideoExportError,
  chooseCompositeRecordingFormat,
  exportCompositeVideo,
  resolveCompositeRecordingFormat,
  type CompositeVideoDependencies,
  type CompositeRecorder,
} from './composite-video'
import type { CompositeCanvasVisualTokens } from './composite-visual-tokens'

const visualTokens: CompositeCanvasVisualTokens = {
  canvas: 'rgb(8, 11, 16)',
  cameraSurface: 'rgb(17, 23, 32)',
  labelScrim: 'rgb(0, 0, 0, 0.72)',
  labelText: 'rgb(255, 255, 255)',
  labelFont: '600 18px Inter, sans-serif',
  labelInset: 8,
  labelPaddingInline: 8,
  labelHeight: 24,
  labelBaseline: 17,
}

function fileRef(name: string): LocalFileRef {
  return {
    name,
    relativePath: `SavedClips/event/${name}`,
    size: 4,
    type: 'video/mp4',
    getFile: async () => new File(['clip'], name, { type: 'video/mp4' }),
  }
}

function event(file = fileRef('front.mp4')): TeslaCamEvent {
  return {
    id: 'event',
    category: 'saved',
    timestamp: '2026-08-21_12-00-00',
    segments: [{ timestamp: '2026-08-21_12-00-00', cameras: { front: file } }],
  }
}

function dependencies(
  revokeObjectURL: (url: string) => void,
  fillText: (text: string, x: number, y: number) => void = () => undefined,
): CompositeVideoDependencies {
  return {
    createCanvas: () => {
      const canvas = document.createElement('canvas')
      Object.defineProperty(canvas, 'getContext', {
        value: () => ({
          drawImage: () => undefined,
          fillRect: () => undefined,
          fillText,
          measureText: (text: string) => ({ width: text.length * 8 }),
        }),
      })
      Object.defineProperty(canvas, 'captureStream', { value: () => ({ getTracks: () => [] }) })
      return canvas
    },
    createVideo: () => {
      const video = document.createElement('video')
      Object.defineProperty(video, 'load', { value: () => undefined })
      return video
    },
    createImage: () => loadedVehicleImage(),
    createObjectURL: () => 'blob:front',
    revokeObjectURL,
    isTypeSupported: () => true,
    createRecorder: () => {
      throw new Error('recorder must not be created')
    },
    requestAnimationFrame: () => 1,
    cancelAnimationFrame: () => undefined,
    now: () => 0,
    isPageVisible: () => true,
    subscribePageVisibility: () => () => undefined,
    readCompositeCanvasVisualTokens: () => visualTokens,
  }
}

function loadedVehicleImage(): HTMLImageElement {
  const image = document.createElement('img')
  Object.defineProperties(image, {
    naturalWidth: { value: 360 },
    naturalHeight: { value: 720 },
    complete: { value: true },
  })
  return image
}

function loadedVideo(play: () => Promise<void>): HTMLVideoElement {
  const video = document.createElement('video')
  Object.defineProperties(video, {
    duration: { value: 0.01 },
    videoWidth: { value: 1920 },
    videoHeight: { value: 1080 },
    readyState: { value: HTMLMediaElement.HAVE_CURRENT_DATA },
    load: { value: () => video.onloadedmetadata?.(new Event('loadedmetadata')) },
    play: { value: play },
    pause: { value: () => undefined },
  })
  return video
}

function compositeRecorder(): CompositeRecorder {
  let state: RecordingState = 'inactive'
  const recorder: CompositeRecorder = {
    mimeType: 'video/webm',
    get state() { return state },
    ondataavailable: null,
    onerror: null,
    onpause: null,
    onresume: null,
    onstop: null,
    start: () => { state = 'recording' },
    pause: () => {
      state = 'paused'
      recorder.onpause?.(new Event('pause'))
    },
    resume: () => {
      state = 'recording'
      recorder.onresume?.(new Event('resume'))
    },
    stop: () => {
      state = 'inactive'
      recorder.ondataavailable?.(new BlobEvent('dataavailable', { data: new Blob(['video']) }))
      recorder.onstop?.(new Event('stop'))
    },
  }
  return recorder
}

describe('chooseCompositeRecordingFormat', () => {
  it('reports MP4 when AVC WebCodecs export is available without MediaRecorder', async () => {
    await expect(resolveCompositeRecordingFormat(event(), {
      canUseAcceleratedExport: async () => true,
      isMediaRecorderAvailable: () => false,
      isTypeSupported: () => false,
    })).resolves.toEqual({
      mimeType: 'video/mp4',
      extension: 'mp4',
      label: 'MP4',
    })
  })

  it('reports the WebM fallback when accelerated MP4 can fail at runtime', async () => {
    await expect(resolveCompositeRecordingFormat(event(), {
      canUseAcceleratedExport: async () => true,
      isMediaRecorderAvailable: () => true,
      isTypeSupported: (mimeType) => mimeType === 'video/webm',
    })).resolves.toEqual({
      mimeType: 'video/mp4',
      extension: 'mp4',
      label: 'MP4',
      fallbackLabel: 'WebM',
    })
  })

  it('reports the WebM fallback when AVC WebCodecs encoding is unavailable', async () => {
    await expect(resolveCompositeRecordingFormat(event(), {
      canUseAcceleratedExport: async () => false,
      isMediaRecorderAvailable: () => true,
      isTypeSupported: (mimeType) => mimeType === 'video/webm',
    })).resolves.toEqual({
      mimeType: 'video/webm',
      extension: 'webm',
      label: 'WebM',
    })
  })

  it('reports no format when neither AVC WebCodecs nor MediaRecorder is available', async () => {
    await expect(resolveCompositeRecordingFormat(event(), {
      canUseAcceleratedExport: async () => false,
      isMediaRecorderAvailable: () => false,
      isTypeSupported: () => false,
    })).resolves.toBeNull()
  })

  it('prefers a supported MP4 format over WebM', () => {
    const supported = vi.fn((mimeType: string) => mimeType.includes('webm') || mimeType === 'video/mp4')

    expect(chooseCompositeRecordingFormat(supported)).toEqual({
      mimeType: 'video/mp4',
      extension: 'mp4',
      label: 'MP4',
    })
  })

  it('falls back to a supported WebM format', () => {
    const supported = (mimeType: string) => mimeType === 'video/webm'

    expect(chooseCompositeRecordingFormat(supported)).toEqual({
      mimeType: 'video/webm',
      extension: 'webm',
      label: 'WebM',
    })
  })
})

describe('exportCompositeVideo', () => {
  it('exports a truthful WebM blob and reports normalized progress', async () => {
    const revokeObjectURL = vi.fn()
    const progress = vi.fn()
    const fillText = vi.fn()
    const recorder = compositeRecorder()
    const successfulDependencies: CompositeVideoDependencies = {
      ...dependencies(revokeObjectURL, fillText),
      createVideo: () => loadedVideo(async () => undefined),
      isTypeSupported: (mimeType) => mimeType === 'video/webm',
      createRecorder: () => recorder,
      requestAnimationFrame: (callback) => {
        callback(20)
        return 1
      },
    }

    const result = await exportCompositeVideo({ event: event(), onProgress: progress, dependencies: successfulDependencies })

    expect(result).toMatchObject({ mimeType: 'video/webm', extension: 'webm' })
    expect(result.blob).toMatchObject({ size: 5, type: 'video/webm' })
    expect(progress.mock.calls.flat()).toEqual([0, 1, 1])
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:front')
    expect(fillText).toHaveBeenCalledWith('リア・記録なし', 416, 517)
  })

  it('resolves composite Canvas visual tokens once before rendering', async () => {
    const recorder = compositeRecorder()
    const readCanvasVisualTokens = vi.fn(() => visualTokens)
    const fillStyles: string[] = []
    const fonts: string[] = []
    const visualDependencies: CompositeVideoDependencies = {
      ...dependencies(() => undefined),
      createCanvas: () => {
        const canvas = document.createElement('canvas')
        Object.defineProperty(canvas, 'getContext', {
          value: () => ({
            drawImage: () => undefined,
            fillRect: () => undefined,
            fillText: () => undefined,
            measureText: (text: string) => ({ width: text.length * 8 }),
            set fillStyle(value: string) { fillStyles.push(value) },
            set font(value: string) { fonts.push(value) },
          }),
        })
        Object.defineProperty(canvas, 'captureStream', { value: () => ({ getTracks: () => [] }) })
        return canvas
      },
      createVideo: () => loadedVideo(async () => undefined),
      createRecorder: () => recorder,
      readCompositeCanvasVisualTokens: readCanvasVisualTokens,
      requestAnimationFrame: (callback) => {
        callback(20)
        return 1
      },
    }

    await expect(exportCompositeVideo({ event: event(), dependencies: visualDependencies })).resolves.toMatchObject({ extension: 'webm' })

    expect(readCanvasVisualTokens).toHaveBeenCalledTimes(1)
    expect(fillStyles).toEqual(expect.arrayContaining([
      visualTokens.canvas,
      visualTokens.cameraSurface,
      visualTokens.labelScrim,
      visualTokens.labelText,
    ]))
    expect(fonts).toContain(visualTokens.labelFont)
  })

  it('loads the public vehicle asset once for a multi-segment export', async () => {
    const recorder = compositeRecorder()
    const images: HTMLImageElement[] = []
    const createImage = vi.fn(() => {
      const image = loadedVehicleImage()
      images.push(image)
      return image
    })
    const baseEvent = event()
    const firstSegment = baseEvent.segments.at(0)
    if (firstSegment === undefined) throw new Error('first segment is missing')
    const twoSegments: TeslaCamEvent = {
      ...baseEvent,
      segments: [firstSegment, { timestamp: '2026-08-21_12-01-00', cameras: { front: fileRef('later-front.mp4') } }],
    }
    const vehicleDependencies: CompositeVideoDependencies = {
      ...dependencies(() => undefined),
      createImage,
      createVideo: () => loadedVideo(async () => undefined),
      createRecorder: () => recorder,
      requestAnimationFrame: (callback) => {
        callback(20)
        return 1
      },
    }

    await expect(exportCompositeVideo({ event: twoSegments, dependencies: vehicleDependencies })).resolves.toMatchObject({ extension: 'webm' })

    expect(createImage).toHaveBeenCalledTimes(1)
    expect(images.at(0)?.src).toContain('/vehicle-top.png')
  })

  it('fails and cleans up when every video refuses playback', async () => {
    const revokeObjectURL = vi.fn()
    const recorder = compositeRecorder()
    const failingDependencies: CompositeVideoDependencies = {
      ...dependencies(revokeObjectURL),
      createVideo: () => loadedVideo(async () => Promise.reject(new DOMException('', 'NotSupportedError'))),
      createRecorder: () => recorder,
    }

    await expect(exportCompositeVideo({ event: event(), dependencies: failingDependencies })).rejects.toMatchObject({
      code: 'recording-failed',
      message: '動画を再生できないため、書き出しを開始できません。',
    })
    expect(recorder.state).toBe('inactive')
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:front')
  })

  it('releases each segment before loading the next and resumes after a skipped segment', async () => {
    const revoked: string[] = []
    const recorder = compositeRecorder()
    const pause = vi.spyOn(recorder, 'pause')
    const resume = vi.spyOn(recorder, 'resume')
    let urlIndex = 0
    const sequentialDependencies: CompositeVideoDependencies = {
      ...dependencies((url) => revoked.push(url)),
      createVideo: () => loadedVideo(async () => undefined),
      createObjectURL: () => {
        if (urlIndex === 1) expect(revoked).toContain('blob:0')
        const url = `blob:${urlIndex}`
        urlIndex += 1
        return url
      },
      createRecorder: () => recorder,
      requestAnimationFrame: (callback) => {
        callback(20)
        return 1
      },
    }
    const baseEvent = event()
    const threeSegments: TeslaCamEvent = {
      ...baseEvent,
      segments: [
        baseEvent.segments[0],
        { timestamp: '2026-08-21_12-01-00', cameras: {} },
        { timestamp: '2026-08-21_12-02-00', cameras: { front: fileRef('later-front.mp4') } },
      ].flatMap((segment) => segment === undefined ? [] : [segment]),
    }

    await expect(exportCompositeVideo({ event: threeSegments, dependencies: sequentialDependencies })).resolves.toMatchObject({ extension: 'webm' })
    expect(pause).toHaveBeenCalledTimes(1)
    expect(resume).toHaveBeenCalledTimes(1)
    expect(revoked).toEqual(['blob:0', 'blob:1'])
    expect(recorder.state).toBe('inactive')
  })

  it('stops recording and stream tracks when aborted during a pause transition', async () => {
    const controller = new AbortController()
    const recorder = compositeRecorder()
    const stopTrack = vi.fn()
    let signalPauseReached: () => void = () => undefined
    const pauseReached = new Promise<void>((resolve) => { signalPauseReached = resolve })
    recorder.pause = () => { signalPauseReached() }
    const abortingDependencies: CompositeVideoDependencies = {
      ...dependencies(() => undefined),
      createCanvas: () => {
        const canvas = document.createElement('canvas')
        Object.defineProperty(canvas, 'getContext', {
          value: () => ({
            drawImage: () => undefined,
            fillRect: () => undefined,
            fillText: () => undefined,
            measureText: (text: string) => ({ width: text.length * 8 }),
          }),
        })
        Object.defineProperty(canvas, 'captureStream', { value: () => ({ getTracks: () => [{ stop: stopTrack }] }) })
        return canvas
      },
      createVideo: () => loadedVideo(async () => undefined),
      createRecorder: () => recorder,
      requestAnimationFrame: (callback) => {
        callback(20)
        return 1
      },
    }
    const baseEvent = event()
    const firstSegment = baseEvent.segments.at(0)
    if (firstSegment === undefined) throw new Error('first segment is missing')
    const exportPromise = exportCompositeVideo({
      event: {
        ...baseEvent,
        segments: [firstSegment, { timestamp: '2026-08-21_12-01-00', cameras: { front: fileRef('later.mp4') } }],
      },
      signal: controller.signal,
      dependencies: abortingDependencies,
    })

    await pauseReached
    controller.abort()

    await expect(exportPromise).rejects.toMatchObject({ code: 'aborted' })
    expect(recorder.state).toBe('inactive')
    expect(stopTrack).toHaveBeenCalledTimes(1)
  })

  it('rejects cancellation before touching event media', async () => {
    const getFile = vi.fn(async () => new File(['clip'], 'front.mp4'))
    const controller = new AbortController()
    controller.abort()

    await expect(exportCompositeVideo({
      event: event({ ...fileRef('front.mp4'), getFile }),
      signal: controller.signal,
      dependencies: dependencies(() => undefined),
    })).rejects.toMatchObject({ code: 'aborted' })
    expect(getFile).not.toHaveBeenCalled()
  })

  it('revokes object URLs when cancellation interrupts media loading', async () => {
    const revokeObjectURL = vi.fn()
    const controller = new AbortController()
    const exportPromise = exportCompositeVideo({
      event: event(),
      signal: controller.signal,
      dependencies: dependencies(revokeObjectURL),
    })

    await Promise.resolve()
    await Promise.resolve()
    controller.abort()

    await expect(exportPromise).rejects.toMatchObject({ code: 'aborted' })
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:front')
  })

  it('reports unsupported recording without reading media', async () => {
    const getFile = vi.fn(async () => new File(['clip'], 'front.mp4'))
    const unsupportedDependencies = {
      ...dependencies(() => undefined),
      isTypeSupported: () => false,
    }

    await expect(exportCompositeVideo({
      event: event({ ...fileRef('front.mp4'), getFile }),
      dependencies: unsupportedDependencies,
    })).rejects.toEqual(new CompositeVideoExportError('unsupported', 'このブラウザは動画書き出しに対応していません。'))
    expect(getFile).not.toHaveBeenCalled()
  })

  it('reports an event with no media as not playable', async () => {
    await expect(exportCompositeVideo({
      event: { ...event(), segments: [] },
      dependencies: dependencies(() => undefined),
    })).rejects.toMatchObject({ code: 'no-playable-media' })
  })
})
