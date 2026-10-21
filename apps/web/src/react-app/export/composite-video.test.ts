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
import type { CompositeLayer } from './composite-frame'

type LayerRecord = {
  readonly fillStyles: string[]
  readonly fonts: string[]
  readonly texts: string[]
}

function recordingLayer(record: LayerRecord = { fillStyles: [], fonts: [], texts: [] }): CompositeLayer {
  let fillStyle: string | CanvasGradient | CanvasPattern = ''
  let font = ''
  return {
    canvas: document.createElement('canvas'),
    context: {
      get fillStyle() { return fillStyle },
      set fillStyle(value) {
        fillStyle = value
        if (typeof value === 'string') record.fillStyles.push(value)
      },
      get font() { return font },
      set font(value) {
        font = value
        record.fonts.push(value)
      },
      globalCompositeOperation: 'source-over',
      textAlign: 'left',
      beginPath: () => undefined,
      closePath: () => undefined,
      moveTo: () => undefined,
      lineTo: () => undefined,
      roundRect: () => undefined,
      fill: () => undefined,
      fillRect: () => undefined,
      drawImage: () => undefined,
      createLinearGradient: () => ({ addColorStop: () => undefined }),
      fillText: (text) => {
        record.texts.push(text)
      },
      measureText: (text) => ({ width: text.length * 8 }),
    },
  }
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
  layerRecord?: LayerRecord,
): CompositeVideoDependencies {
  return {
    createCanvas: () => {
      const canvas = document.createElement('canvas')
      Object.defineProperty(canvas, 'getContext', {
        value: () => ({
          drawImage: () => undefined,
          fillRect: () => undefined,
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
    createLayer: () => recordingLayer(layerRecord),
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
    const layer: LayerRecord = { fillStyles: [], fonts: [], texts: [] }
    const recorder = compositeRecorder()
    const successfulDependencies: CompositeVideoDependencies = {
      ...dependencies(revokeObjectURL, layer),
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
    expect(layer.texts.filter((text) => text === '記録なし')).toHaveLength(5)
    expect(layer.texts).toEqual(expect.arrayContaining(['フロント', 'リア']))
  })

  it('renders the static overlay once and paints it with the contract colours', async () => {
    const recorder = compositeRecorder()
    const layer: LayerRecord = { fillStyles: [], fonts: [], texts: [] }
    const createLayer = vi.fn(() => recordingLayer(layer))
    const overlayDependencies: CompositeVideoDependencies = {
      ...dependencies(() => undefined),
      createVideo: () => loadedVideo(async () => undefined),
      createRecorder: () => recorder,
      createLayer,
      requestAnimationFrame: (callback) => {
        callback(20)
        return 1
      },
    }

    await expect(exportCompositeVideo({ event: event(), dependencies: overlayDependencies })).resolves.toMatchObject({ extension: 'webm' })

    expect(createLayer).toHaveBeenCalledTimes(1)
    expect(layer.fillStyles).toEqual(expect.arrayContaining(['#E9EBEE', '#15181D', '#C2C8D0', '#0000008C', '#FFFFFF']))
    expect(layer.fonts.every((font) => font.startsWith('500 13px "Hiragino Sans"'))).toBe(true)
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
