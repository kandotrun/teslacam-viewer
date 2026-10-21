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
import type { CompositeFrameRenderer, CompositeFrameSource } from './composite-frame'
import { buildCompositeLayout, type CompositeLayout } from './composite-layout'
import {
  compositeFrameDuration,
  compositeFrameRate,
  compositeProgressAfterFrame,
  planCompositeSegment,
} from './composite-plan'
import { CompositeVideoExportError, exportAborted } from './composite-recorder'
import type { CompositeVideoExportResult } from './composite-video'

export type CompositeWebCodecsOptions = {
  readonly event: TeslaCamEvent
  readonly canvas: HTMLCanvasElement
  readonly context: CanvasRenderingContext2D
  readonly renderer: CompositeFrameRenderer
  readonly signal?: AbortSignal | undefined
  readonly onProgress?: ((progress: number) => void) | undefined
}

type DecodedCamera = {
  readonly cameraId: CameraId
  readonly input: Input
  readonly duration: number
  readonly sink: VideoSampleSink
}

type CameraIterator = DecodedCamera & {
  readonly frameCount: number
  readonly samples: AsyncIterator<VideoSample | null>
}

const frameDuration = 1 / compositeFrameRate

function assertActive(signal: AbortSignal | undefined): void {
  if (signal?.aborted === true) throw exportAborted()
}

function recordingFailed(): CompositeVideoExportError {
  return new CompositeVideoExportError('recording-failed', '動画の録画に失敗しました。')
}

function frameTimestamps(frameCount: number): number[] {
  return Array.from({ length: frameCount }, (_, index) => index * frameDuration)
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
  assertActive(options.signal)
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
    return {
      cameraId,
      input,
      duration,
      sink: new VideoSampleSink(track, { hardwareAcceleration: 'no-preference' }),
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

function createIterators(
  cameras: readonly DecodedCamera[],
  frameCounts: Readonly<Partial<Record<CameraId, number>>>,
): readonly CameraIterator[] {
  return cameras.map((camera) => {
    const frameCount = frameCounts[camera.cameraId] ?? 0
    return {
      ...camera,
      frameCount,
      samples: camera.sink.samplesAtTimestamps(frameTimestamps(frameCount))[Symbol.asyncIterator](),
    }
  })
}

/** Decodes each camera's sample for this output frame and draws it straight into the output canvas. */
async function drawFrame(
  options: CompositeWebCodecsOptions,
  cameras: readonly CameraIterator[],
  frameIndex: number,
): Promise<void> {
  const samples = await Promise.all(cameras.map(async (camera) => {
    if (frameIndex >= camera.frameCount) return null
    const result = await camera.samples.next()
    return result.done === true ? null : result.value
  }))
  try {
    const sources = samples.flatMap((sample, index): CompositeFrameSource[] => {
      const camera = cameras[index]
      if (sample === null || camera === undefined) return []
      return [{
        cameraId: camera.cameraId,
        width: sample.displayWidth,
        height: sample.displayHeight,
        draw: (rect) => sample.draw(options.context, rect.x, rect.y, rect.width, rect.height),
      }]
    })
    options.renderer.drawFrame(options.context, sources)
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
  output.addVideoTrack(canvasSource, { frameRate: compositeFrameRate })
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
        const plan = planCompositeSegment(
          Object.fromEntries(cameras.map((camera) => [camera.cameraId, camera.duration])),
          outputTimestamp,
        )
        if (plan.skipped) continue
        const iterators = createIterators(cameras, plan.cameraFrameCounts)
        for (let frameIndex = 0; frameIndex < plan.frameCount; frameIndex += 1) {
          assertActive(options.signal)
          await drawFrame(options, iterators, frameIndex)
          await canvasSource.add(
            plan.startSeconds + frameIndex * frameDuration,
            compositeFrameDuration(frameIndex, plan.durationSeconds),
          )
          addedFrames += 1
          const progress = compositeProgressAfterFrame(segmentIndex, frameIndex, plan.frameCount, options.event.segments.length)
          if (progress !== null) options.onProgress?.(progress)
        }
        outputTimestamp += plan.durationSeconds
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
