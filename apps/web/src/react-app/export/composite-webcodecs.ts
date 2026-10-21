import {
  BlobSource,
  BufferTarget,
  canEncodeVideo,
  CanvasSource,
  Input,
  MP4,
  Mp4OutputFormat,
  Output,
  Quality,
  VideoSampleSink,
  type VideoSample,
} from 'mediabunny'

import { cameraIds, type CameraId, type LocalFileRef, type TeslaCamEvent } from '../../teslacam/types'
import {
  buildCompositeLayout,
  drawCompositeFrame,
  type CompositeDrawable,
  type CompositeLayout,
} from './composite-layout'
import { CompositeVideoExportError, exportAborted } from './composite-recorder'
import type { CompositeCanvasVisualTokens } from './composite-visual-tokens'
import type { CompositeVideoExportResult } from './composite-video'

export type CompositeWebCodecsOptions = {
  readonly event: TeslaCamEvent
  readonly canvas: HTMLCanvasElement
  readonly context: CanvasRenderingContext2D
  readonly vehicleImage: HTMLImageElement
  readonly visualTokens: CompositeCanvasVisualTokens
  readonly createCanvas: () => HTMLCanvasElement
  readonly signal?: AbortSignal | undefined
  readonly onProgress?: ((progress: number) => void) | undefined
}

type DecodedCamera = {
  readonly cameraId: CameraId
  readonly input: Input
  readonly duration: number
  readonly sink: VideoSampleSink
  readonly canvas: HTMLCanvasElement
  readonly context: CanvasRenderingContext2D
}

type CameraIterator = DecodedCamera & {
  readonly frameCount: number
  readonly samples: AsyncIterator<VideoSample | null>
}

const frameRate = 30
const frameDuration = 1 / frameRate

function assertActive(signal: AbortSignal | undefined): void {
  if (signal?.aborted === true) throw exportAborted()
}

function recordingFailed(): CompositeVideoExportError {
  return new CompositeVideoExportError('recording-failed', '動画の録画に失敗しました。')
}

function frameTimestamps(duration: number): number[] {
  return Array.from({ length: Math.ceil(duration * frameRate) }, (_, index) => index * frameDuration)
}

async function canEncodeComposite(layout: CompositeLayout): Promise<boolean> {
  if (typeof VideoEncoder === 'undefined' || typeof VideoDecoder === 'undefined') return false
  return canEncodeVideo('avc', {
    width: layout.width,
    height: layout.height,
    quality: new Quality('high'),
    latencyMode: 'quality',
    hardwareAcceleration: 'no-preference',
  })
}

async function canDecodeFile(fileRef: LocalFileRef): Promise<boolean> {
  let input: Input | undefined
  try {
    input = new Input({ source: new BlobSource(await fileRef.getFile()), formats: [MP4] })
    const track = await input.getPrimaryVideoTrack()
    return track !== null && await track.canDecode()
  } catch {
    return false
  } finally {
    input?.dispose()
  }
}

export async function canExportCompositeWithWebCodecs(event: TeslaCamEvent): Promise<boolean> {
  if (!await canEncodeComposite(buildCompositeLayout())) return false
  const candidates = event.segments.flatMap(({ cameras }) => cameraIds.flatMap((cameraId) => {
    const fileRef = cameras[cameraId]
    return fileRef === undefined ? [] : [fileRef]
  })).slice(0, cameraIds.length)
  for (const candidate of candidates) {
    if (await canDecodeFile(candidate)) return true
  }
  return false
}

async function prepareCamera(
  cameraId: CameraId,
  fileRef: LocalFileRef,
  options: CompositeWebCodecsOptions,
): Promise<DecodedCamera | null> {
  const file = await fileRef.getFile()
  assertActive(options.signal)
  const input = new Input({ source: new BlobSource(file), formats: [MP4] })
  try {
    const track = await input.getPrimaryVideoTrack()
    if (track === null || !await track.canDecode()) {
      input.dispose()
      return null
    }
    const metadataDuration = await track.getDurationFromMetadata()
    const duration = metadataDuration ?? await track.computeDuration()
    if (!Number.isFinite(duration) || duration <= 0) {
      input.dispose()
      return null
    }
    const canvas = options.createCanvas()
    const context = canvas.getContext('2d')
    if (context === null) {
      input.dispose()
      return null
    }
    return {
      cameraId,
      input,
      duration,
      sink: new VideoSampleSink(track, { hardwareAcceleration: 'no-preference' }),
      canvas,
      context,
    }
  } catch {
    input.dispose()
    return null
  }
}

async function prepareSegment(
  options: CompositeWebCodecsOptions,
  segmentIndex: number,
): Promise<readonly DecodedCamera[]> {
  const segment = options.event.segments.at(segmentIndex)
  if (segment === undefined) return []
  const pending = cameraIds.flatMap((cameraId) => {
    const fileRef = segment.cameras[cameraId]
    return fileRef === undefined ? [] : [prepareCamera(cameraId, fileRef, options)]
  })
  const settled = await Promise.allSettled(pending)
  const cameras = settled.flatMap((result) => result.status === 'fulfilled' && result.value !== null ? [result.value] : [])
  if (options.signal?.aborted === true) {
    for (const { input } of cameras) input.dispose()
    throw exportAborted()
  }
  return cameras
}

function createIterators(cameras: readonly DecodedCamera[]): readonly CameraIterator[] {
  return cameras.map((camera) => {
    const timestamps = frameTimestamps(camera.duration)
    return {
      ...camera,
      frameCount: timestamps.length,
      samples: camera.sink.samplesAtTimestamps(timestamps)[Symbol.asyncIterator](),
    }
  })
}

async function drawFrame(
  options: CompositeWebCodecsOptions,
  layout: CompositeLayout,
  cameras: readonly CameraIterator[],
  frameIndex: number,
): Promise<void> {
  const samples = await Promise.all(cameras.map(async (camera) => {
    if (frameIndex >= camera.frameCount) return null
    const result = await camera.samples.next()
    return result.done === true ? null : result.value
  }))
  const drawables: CompositeDrawable[] = []
  try {
    for (const [index, sample] of samples.entries()) {
      if (sample === null) continue
      const camera = cameras[index]
      if (camera === undefined) continue
      if (camera.canvas.width !== sample.displayWidth) camera.canvas.width = sample.displayWidth
      if (camera.canvas.height !== sample.displayHeight) camera.canvas.height = sample.displayHeight
      camera.context.clearRect(0, 0, camera.canvas.width, camera.canvas.height)
      sample.draw(camera.context, 0, 0, camera.canvas.width, camera.canvas.height)
      drawables.push({
        cameraId: camera.cameraId,
        source: camera.canvas,
        width: camera.canvas.width,
        height: camera.canvas.height,
        playable: true,
      })
    }
    drawCompositeFrame(
      options.context,
      layout,
      drawables,
      options.vehicleImage,
      options.visualTokens,
    )
  } finally {
    for (const sample of samples) sample?.close()
  }
}

async function cancelOutput(output: Output<Mp4OutputFormat, BufferTarget>): Promise<void> {
  if (output.state === 'pending' || output.state === 'started') await output.cancel()
}

export async function tryExportCompositeWithWebCodecs(
  options: CompositeWebCodecsOptions,
): Promise<CompositeVideoExportResult | null> {
  const layout = buildCompositeLayout()
  const quality = new Quality('high')
  if (!await canEncodeComposite(layout)) return null
  const target = new BufferTarget()
  const output = new Output({ format: new Mp4OutputFormat(), target })
  const canvasSource = new CanvasSource(options.canvas, {
    codec: 'avc',
    quality,
    keyFrameInterval: 2,
    latencyMode: 'quality',
    hardwareAcceleration: 'no-preference',
  })
  output.addVideoTrack(canvasSource, { frameRate })
  let addedFrames = 0
  let outputTimestamp = 0
  try {
    await output.start()
    options.onProgress?.(0)
    for (const [segmentIndex] of options.event.segments.entries()) {
      assertActive(options.signal)
      const cameras = await prepareSegment(options, segmentIndex)
      const handleAbort = () => {
        for (const { input } of cameras) input.dispose()
      }
      options.signal?.addEventListener('abort', handleAbort, { once: true })
      try {
        if (cameras.length === 0) continue
        const duration = Math.max(...cameras.map((camera) => camera.duration))
        const frameCount = Math.ceil(duration * frameRate)
        const iterators = createIterators(cameras)
        for (let frameIndex = 0; frameIndex < frameCount; frameIndex += 1) {
          assertActive(options.signal)
          await drawFrame(options, layout, iterators, frameIndex)
          const timestamp = outputTimestamp + frameIndex * frameDuration
          const durationRemaining = Math.max(0, duration - frameIndex * frameDuration)
          await canvasSource.add(timestamp, Math.min(frameDuration, durationRemaining))
          addedFrames += 1
          if (frameIndex % frameRate === 0 || frameIndex === frameCount - 1) {
            options.onProgress?.((segmentIndex + (frameIndex + 1) / frameCount) / options.event.segments.length)
          }
        }
        outputTimestamp += duration
      } finally {
        options.signal?.removeEventListener('abort', handleAbort)
        for (const { input } of cameras) input.dispose()
      }
    }
    if (addedFrames === 0) {
      await cancelOutput(output)
      return null
    }
    await output.finalize()
    assertActive(options.signal)
    if (target.buffer === null) throw recordingFailed()
    options.onProgress?.(1)
    const mimeType = await output.getMimeType()
    return {
      blob: new Blob([target.buffer], { type: mimeType }),
      mimeType,
      extension: 'mp4',
    }
  } catch (error) {
    await cancelOutput(output)
    if (options.signal?.aborted === true) throw exportAborted()
    if (error instanceof CompositeVideoExportError) throw error
    throw recordingFailed()
  }
}
