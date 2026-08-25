import { cameraIds, type CameraId, type LocalFileRef, type TeslaCamEvent } from '../../teslacam/types'
import { buildCompositeLayout } from './composite-layout'
import { loadCompositeVehicleImage } from './composite-asset'
import {
  CompositeRecorderLifecycle,
  CompositeVideoExportError,
  exportAborted,
  type CompositeRecorder,
} from './composite-recorder'
import { renderCompositeSegment, type CompositeRenderClip } from './composite-renderer'
import { readCompositeCanvasVisualTokens, type CompositeCanvasVisualTokens } from './composite-visual-tokens'

export { CompositeVideoExportError } from './composite-recorder'
export type { CompositeRecorder, CompositeVideoExportErrorCode } from './composite-recorder'

export type CompositeRecordingFormat = {
  readonly mimeType: string
  readonly extension: 'mp4' | 'webm'
  readonly label: 'MP4' | 'WebM'
  readonly fallbackLabel?: 'MP4' | 'WebM'
}

export type CompositeFormatDependencies = {
  readonly canUseAcceleratedExport: (event: TeslaCamEvent) => Promise<boolean>
  readonly isMediaRecorderAvailable: () => boolean
  readonly isTypeSupported: (mimeType: string) => boolean
}

export type CompositeVideoDependencies = {
  readonly createCanvas: () => HTMLCanvasElement; readonly createVideo: () => HTMLVideoElement; readonly createImage: () => HTMLImageElement
  readonly createObjectURL: (file: File) => string; readonly revokeObjectURL: (url: string) => void
  readonly isTypeSupported: (mimeType: string) => boolean
  readonly createRecorder: (stream: MediaStream, mimeType: string) => CompositeRecorder
  readonly requestAnimationFrame: (callback: FrameRequestCallback) => number; readonly cancelAnimationFrame: (handle: number) => void
  readonly now: () => number
  readonly isPageVisible: () => boolean
  readonly subscribePageVisibility: (listener: (visible: boolean) => void) => () => void
  readonly readCompositeCanvasVisualTokens: () => CompositeCanvasVisualTokens
}

export type CompositeVideoExportOptions = { readonly event: TeslaCamEvent; readonly signal?: AbortSignal; readonly onProgress?: (progress: number) => void; readonly dependencies?: CompositeVideoDependencies }

export type CompositeVideoExportResult = { readonly blob: Blob; readonly mimeType: string; readonly extension: 'mp4' | 'webm' }

type PreparedClip = CompositeRenderClip

type PreparedSegment = { readonly clips: readonly PreparedClip[]; readonly duration: number }

type PreparationContext = {
  readonly dependencies: CompositeVideoDependencies; readonly signal: AbortSignal | undefined; readonly objectUrls: string[]
}

const recordingFormats = [
  { mimeType: 'video/mp4;codecs=avc1.42E01E,mp4a.40.2', extension: 'mp4', label: 'MP4' },
  { mimeType: 'video/mp4', extension: 'mp4', label: 'MP4' },
  { mimeType: 'video/webm;codecs=vp9,opus', extension: 'webm', label: 'WebM' },
  { mimeType: 'video/webm;codecs=vp8,opus', extension: 'webm', label: 'WebM' },
  { mimeType: 'video/webm', extension: 'webm', label: 'WebM' },
] as const satisfies readonly CompositeRecordingFormat[]

const acceleratedFormat: CompositeRecordingFormat = { mimeType: 'video/mp4', extension: 'mp4', label: 'MP4' }

function browserDependencies(): CompositeVideoDependencies {
  return {
    createCanvas: () => document.createElement('canvas'),
    createVideo: () => document.createElement('video'),
    createImage: () => new Image(),
    createObjectURL: (file) => URL.createObjectURL(file),
    revokeObjectURL: (url) => URL.revokeObjectURL(url),
    isTypeSupported: (mimeType) => typeof MediaRecorder !== 'undefined' && MediaRecorder.isTypeSupported(mimeType),
    createRecorder: (stream, mimeType) => new MediaRecorder(stream, { mimeType }),
    requestAnimationFrame: (callback) => window.requestAnimationFrame(callback),
    cancelAnimationFrame: (handle) => window.cancelAnimationFrame(handle),
    now: () => performance.now(),
    isPageVisible: () => document.visibilityState === 'visible',
    subscribePageVisibility: (listener) => {
      const handleVisibility = () => listener(document.visibilityState === 'visible')
      document.addEventListener('visibilitychange', handleVisibility)
      return () => document.removeEventListener('visibilitychange', handleVisibility)
    },
    readCompositeCanvasVisualTokens: () => readCompositeCanvasVisualTokens(getComputedStyle(document.documentElement)),
  }
}

function assertActive(signal: AbortSignal | undefined): void {
  if (signal?.aborted === true) throw exportAborted()
}

export function chooseCompositeRecordingFormat(
  isTypeSupported: (mimeType: string) => boolean,
): CompositeRecordingFormat | null {
  return recordingFormats.find(({ mimeType }) => isTypeSupported(mimeType)) ?? null
}

function browserFormatDependencies(): CompositeFormatDependencies {
  return {
    canUseAcceleratedExport: async (event) => {
      const { canExportCompositeWithWebCodecs } = await import('./composite-webcodecs')
      return canExportCompositeWithWebCodecs(event)
    },
    isMediaRecorderAvailable: () => typeof MediaRecorder !== 'undefined',
    isTypeSupported: (mimeType) => typeof MediaRecorder !== 'undefined' && MediaRecorder.isTypeSupported(mimeType),
  }
}

export function compositeExportPotentiallySupported(): boolean {
  return (typeof VideoEncoder !== 'undefined' && typeof VideoDecoder !== 'undefined') || typeof MediaRecorder !== 'undefined'
}

export async function resolveCompositeRecordingFormat(
  event: TeslaCamEvent,
  dependencies: CompositeFormatDependencies = browserFormatDependencies(),
): Promise<CompositeRecordingFormat | null> {
  const accelerated = await dependencies.canUseAcceleratedExport(event).catch(() => false)
  if (accelerated) {
    if (!dependencies.isMediaRecorderAvailable()) return acceleratedFormat
    const fallback = chooseCompositeRecordingFormat(dependencies.isTypeSupported)
    return fallback !== null && fallback.label !== acceleratedFormat.label
      ? { ...acceleratedFormat, fallbackLabel: fallback.label }
      : acceleratedFormat
  }
  if (!dependencies.isMediaRecorderAvailable()) return null
  return chooseCompositeRecordingFormat(dependencies.isTypeSupported)
}

function waitForMetadata(video: HTMLVideoElement, signal: AbortSignal | undefined): Promise<void> {
  return new Promise((resolve, reject) => {
    const finish = (result: 'loaded' | 'error' | 'aborted') => {
      video.onloadedmetadata = null
      video.onerror = null
      signal?.removeEventListener('abort', onAbort)
      if (result === 'loaded') resolve()
      if (result === 'error') reject(new CompositeVideoExportError('no-playable-media', '動画を読み込めませんでした。'))
      if (result === 'aborted') reject(exportAborted())
    }
    const onAbort = () => finish('aborted')
    video.onloadedmetadata = () => finish('loaded')
    video.onerror = () => finish('error')
    signal?.addEventListener('abort', onAbort, { once: true })
    video.load()
  })
}

async function prepareClip(
  cameraId: CameraId,
  fileRef: LocalFileRef,
  preparation: PreparationContext,
): Promise<PreparedClip> {
  const file = await fileRef.getFile()
  assertActive(preparation.signal)
  const url = preparation.dependencies.createObjectURL(file)
  preparation.objectUrls.push(url)
  const video = preparation.dependencies.createVideo()
  video.preload = 'metadata'
  video.muted = true
  video.playsInline = true
  video.src = url
  await waitForMetadata(video, preparation.signal)
  if (!Number.isFinite(video.duration) || video.duration <= 0) {
    throw new CompositeVideoExportError('no-playable-media', '再生可能な動画がありません。')
  }
  return { cameraId, video, duration: video.duration }
}

async function prepareSegment(
  event: TeslaCamEvent,
  segmentIndex: number,
  preparation: PreparationContext,
): Promise<PreparedSegment> {
  const segment = event.segments.at(segmentIndex)
  if (segment === undefined) return { clips: [], duration: 0 }
  const pending = cameraIds.flatMap((cameraId) => {
    const fileRef = segment.cameras[cameraId]
    return fileRef === undefined ? [] : [prepareClip(cameraId, fileRef, preparation)]
  })
  const settled = await Promise.allSettled(pending)
  assertActive(preparation.signal)
  const clips = settled.flatMap((result) => result.status === 'fulfilled' ? [result.value] : [])
  return { clips, duration: Math.max(0, ...clips.map(({ duration }) => duration)) }
}

function outputFormat(mimeType: string, fallback: CompositeRecordingFormat): CompositeRecordingFormat {
  if (mimeType.startsWith('video/mp4')) return { mimeType, extension: 'mp4', label: 'MP4' }
  if (mimeType.startsWith('video/webm')) return { mimeType, extension: 'webm', label: 'WebM' }
  return fallback
}

export async function exportCompositeVideo(options: CompositeVideoExportOptions): Promise<CompositeVideoExportResult> {
  assertActive(options.signal)
  const dependencies = options.dependencies ?? browserDependencies()
  const format = chooseCompositeRecordingFormat(dependencies.isTypeSupported)
  if (options.dependencies !== undefined && format === null) {
    throw new CompositeVideoExportError('unsupported', 'このブラウザは動画書き出しに対応していません。')
  }
  const layout = buildCompositeLayout()
  const canvas = dependencies.createCanvas()
  canvas.width = layout.width
  canvas.height = layout.height
  const context = canvas.getContext('2d')
  if (context === null) throw new CompositeVideoExportError('unsupported', 'このブラウザはCanvas録画に対応していません。')
  const visualTokens = dependencies.readCompositeCanvasVisualTokens()
  const vehicleImage = await loadCompositeVehicleImage(dependencies, options.signal)
  if (options.dependencies === undefined) {
    try {
      const { tryExportCompositeWithWebCodecs } = await import('./composite-webcodecs')
      const accelerated = await tryExportCompositeWithWebCodecs({
        event: options.event,
        canvas,
        context,
        vehicleImage,
        visualTokens,
        createCanvas: dependencies.createCanvas,
        signal: options.signal,
        onProgress: options.onProgress,
      })
      if (accelerated !== null) return accelerated
    } catch (error) {
      if (options.signal?.aborted === true) throw exportAborted()
      if (error instanceof CompositeVideoExportError && error.code === 'aborted') throw error
    }
  }
  if (format === null) throw new CompositeVideoExportError('unsupported', 'このブラウザは動画書き出しに対応していません。')
  if (typeof canvas.captureStream !== 'function') {
    throw new CompositeVideoExportError('unsupported', 'このブラウザはCanvas録画に対応していません。')
  }
  let lifecycle: CompositeRecorderLifecycle | undefined
  let stream: MediaStream | undefined
  let paused = false
  try {
    options.onProgress?.(0)
    for (const [segmentIndex] of options.event.segments.entries()) {
      const objectUrls: string[] = []
      let segment: PreparedSegment | undefined
      try {
        segment = await prepareSegment(options.event, segmentIndex, { dependencies, signal: options.signal, objectUrls })
        if (segment.duration <= 0) continue
        if (lifecycle === undefined) {
          stream = canvas.captureStream(30)
          lifecycle = new CompositeRecorderLifecycle(dependencies.createRecorder(stream, format.mimeType), options.signal)
          lifecycle.start()
        } else if (paused) {
          await lifecycle.resume()
          paused = false
        }
        await renderCompositeSegment({
          clips: segment.clips,
          context,
          vehicleImage,
          segmentIndex,
          segmentCount: options.event.segments.length,
          recorder: lifecycle,
          runtime: dependencies,
          visualTokens,
          signal: options.signal,
          onProgress: options.onProgress,
        })
        if (segmentIndex < options.event.segments.length - 1) {
          await lifecycle.pause()
          paused = true
        }
      } finally {
        for (const { video } of segment?.clips ?? []) video.pause()
        for (const url of objectUrls) dependencies.revokeObjectURL(url)
      }
    }
    if (lifecycle === undefined) {
      throw new CompositeVideoExportError('no-playable-media', '再生可能な動画がありません。')
    }
    await lifecycle.stop()
    options.onProgress?.(1)
    const actualFormat = outputFormat(lifecycle.mimeType, format)
    return {
      blob: new Blob(lifecycle.chunks, { type: actualFormat.mimeType }),
      mimeType: actualFormat.mimeType,
      extension: actualFormat.extension,
    }
  } finally {
    try {
      lifecycle?.forceStop()
    } finally {
      lifecycle?.dispose()
      for (const track of stream?.getTracks() ?? []) track.stop()
    }
  }
}
