import type { CameraId } from '../../teslacam/types'
import type { CompositeFrameSource } from './composite-frame'
import { CompositeVideoExportError, exportAborted } from './composite-recorder'

export type CompositeRenderClip = {
  readonly cameraId: CameraId
  readonly video: HTMLVideoElement
  readonly duration: number
}

export type CompositeRenderRecorder = {
  readonly pause: () => Promise<void>
  readonly resume: () => Promise<void>
  readonly subscribeFailure: (listener: (error: CompositeVideoExportError) => void) => () => void
}

export type CompositeRenderRuntime = {
  readonly requestAnimationFrame: (callback: FrameRequestCallback) => number
  readonly cancelAnimationFrame: (handle: number) => void
  readonly now: () => number
  readonly isPageVisible: () => boolean
  readonly subscribePageVisibility: (listener: (visible: boolean) => void) => () => void
}

export type CompositeRenderOptions = {
  readonly clips: readonly CompositeRenderClip[]
  /** Draws one output frame from the given camera frames (see `createCompositeFrameRenderer`). */
  readonly drawFrame: (sources: readonly CompositeFrameSource[]) => void
  /** Draws a video element into a rectangle of the output canvas. */
  readonly drawVideo: (video: HTMLVideoElement, x: number, y: number, width: number, height: number) => void
  readonly segmentIndex: number
  readonly segmentCount: number
  readonly recorder: CompositeRenderRecorder
  readonly runtime: CompositeRenderRuntime
  readonly signal?: AbortSignal | undefined
  readonly onProgress?: ((progress: number) => void) | undefined
}

const frameIntervalMilliseconds = 1000 / 30
const progressIntervalMilliseconds = 250

function playbackFailed(): CompositeVideoExportError {
  return new CompositeVideoExportError('recording-failed', '動画を再生できないため、書き出しを開始できません。')
}

async function playableClips(clips: readonly CompositeRenderClip[]): Promise<readonly CompositeRenderClip[]> {
  const settled = await Promise.allSettled(clips.map(({ video }) => video.play()))
  return clips.filter((_, index) => settled[index]?.status === 'fulfilled')
}

export async function renderCompositeSegment(options: CompositeRenderOptions): Promise<void> {
  let activeClips = await playableClips(options.clips)
  if (activeClips.length === 0) throw playbackFailed()
  const duration = Math.max(...activeClips.map((clip) => clip.duration))

  await new Promise<void>((resolve, reject) => {
    let frameHandle = 0
    let settled = false
    let visible = options.runtime.isPageVisible()
    let logicalMilliseconds = 0
    let previousTimestamp: number | undefined = options.runtime.now()
    let lastDrawMilliseconds = Number.NEGATIVE_INFINITY
    let lastProgressMilliseconds = Number.NEGATIVE_INFINITY
    let visibilityTransition = Promise.resolve()
    let unsubscribeVisibility: () => void = () => undefined
    let unsubscribeFailure: () => void = () => undefined

    const pauseVideos = () => {
      for (const { video } of activeClips) video.pause()
    }
    const cleanup = () => {
      options.runtime.cancelAnimationFrame(frameHandle)
      unsubscribeVisibility()
      unsubscribeFailure()
      options.signal?.removeEventListener('abort', handleAbort)
      pauseVideos()
    }
    const fail = (error: CompositeVideoExportError) => {
      if (settled) return
      settled = true
      cleanup()
      reject(error)
    }
    const complete = () => {
      if (settled) return
      settled = true
      cleanup()
      resolve()
    }
    const scheduleFrame = () => {
      if (!settled && visible) frameHandle = options.runtime.requestAnimationFrame(render)
    }
    const render = (timestamp: number) => {
      if (settled || !visible) return
      if (previousTimestamp !== undefined) {
        logicalMilliseconds = Math.min(duration * 1000, logicalMilliseconds + Math.max(0, timestamp - previousTimestamp))
      }
      previousTimestamp = timestamp
      if (logicalMilliseconds - lastDrawMilliseconds >= frameIntervalMilliseconds) {
        const sources = activeClips.flatMap(({ cameraId, video }): CompositeFrameSource[] => (
          video.readyState >= HTMLMediaElement.HAVE_CURRENT_DATA && !video.ended
            ? [{
                cameraId,
                width: video.videoWidth,
                height: video.videoHeight,
                draw: (rect) => options.drawVideo(video, rect.x, rect.y, rect.width, rect.height),
              }]
            : []
        ))
        options.drawFrame(sources)
        lastDrawMilliseconds = logicalMilliseconds
      }
      const finished = logicalMilliseconds >= duration * 1000
      if (logicalMilliseconds - lastProgressMilliseconds >= progressIntervalMilliseconds || finished) {
        options.onProgress?.(Math.min(1, (options.segmentIndex + logicalMilliseconds / (duration * 1000)) / options.segmentCount))
        lastProgressMilliseconds = logicalMilliseconds
      }
      if (finished) complete()
      else scheduleFrame()
    }
    const handleTransitionFailure = (error: unknown) => {
      if (error instanceof CompositeVideoExportError) fail(error)
      else if (error instanceof Error) fail(new CompositeVideoExportError('recording-failed', '動画の録画に失敗しました。'))
      else throw error
    }
    const handleVisibility = (nextVisible: boolean) => {
      if (settled || nextVisible === visible) return
      visible = nextVisible
      previousTimestamp = undefined
      options.runtime.cancelAnimationFrame(frameHandle)
      if (!nextVisible) pauseVideos()
      visibilityTransition = visibilityTransition.then(async () => {
        if (settled) return
        if (!nextVisible) {
          await options.recorder.pause()
          return
        }
        await options.recorder.resume()
        activeClips = await playableClips(activeClips)
        if (activeClips.length === 0) throw playbackFailed()
        scheduleFrame()
      })
      visibilityTransition.catch(handleTransitionFailure)
    }
    const handleAbort = () => fail(exportAborted())
    unsubscribeVisibility = options.runtime.subscribePageVisibility(handleVisibility)
    unsubscribeFailure = options.recorder.subscribeFailure(fail)
    options.signal?.addEventListener('abort', handleAbort, { once: true })
    if (options.signal?.aborted === true) {
      fail(exportAborted())
      return
    }
    if (visible) scheduleFrame()
    else {
      visible = true
      handleVisibility(false)
    }
  })
}
