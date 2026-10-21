/**
 * Frame timing of a composite export (parity spec §3.4–§3.6): segments are concatenated by their real
 * media durations, camera-less segments are skipped, and progress is reported once per output second.
 */
import type { CameraId } from '../../teslacam/types'

/** Decodable duration of each camera present in a segment; `null` = present but not decodable. */
export type SegmentCameraDurations = Readonly<Partial<Record<CameraId, number | null>>>

export type CompositeSegmentPlan = {
  readonly skipped: boolean
  readonly durationSeconds: number
  readonly frameCount: number
  readonly startSeconds: number
  readonly cameraFrameCounts: Readonly<Partial<Record<CameraId, number>>>
}

export type CompositeExportPlan =
  | {
      readonly segments: readonly CompositeSegmentPlan[]
      readonly totalFrames: number
      readonly totalDurationSeconds: number
      readonly progress: readonly number[]
    }
  | { readonly error: 'no-playable-media' }

export const compositeFrameRate = 30

function decodable(duration: number | null | undefined): duration is number {
  return duration !== null && duration !== undefined && Number.isFinite(duration) && duration > 0
}

export function planCompositeSegment(cameras: SegmentCameraDurations, startSeconds: number): CompositeSegmentPlan {
  const cameraFrameCounts: Partial<Record<CameraId, number>> = {}
  let durationSeconds = 0
  for (const [cameraId, duration] of Object.entries(cameras)) {
    if (!decodable(duration) || !isCameraId(cameraId, cameras)) continue
    cameraFrameCounts[cameraId] = Math.ceil(duration * compositeFrameRate)
    durationSeconds = Math.max(durationSeconds, duration)
  }
  const skipped = durationSeconds === 0
  return {
    skipped,
    durationSeconds,
    frameCount: skipped ? 0 : Math.ceil(durationSeconds * compositeFrameRate),
    startSeconds,
    cameraFrameCounts,
  }
}

function isCameraId(value: string, cameras: SegmentCameraDurations): value is CameraId {
  return Object.hasOwn(cameras, value)
}

/** Duration of output frame `frameIndex` (the last frame of a segment may be shorter). */
export function compositeFrameDuration(frameIndex: number, segmentDurationSeconds: number): number {
  const frameSeconds = 1 / compositeFrameRate
  return Math.min(frameSeconds, Math.max(0, segmentDurationSeconds - frameIndex * frameSeconds))
}

/** Progress after writing a frame, or `null` when this frame is not a report point. */
export function compositeProgressAfterFrame(
  segmentIndex: number,
  frameIndex: number,
  frameCount: number,
  segmentCount: number,
): number | null {
  if (frameIndex % compositeFrameRate !== 0 && frameIndex !== frameCount - 1) return null
  return (segmentIndex + (frameIndex + 1) / frameCount) / segmentCount
}

export function planCompositeExport(segments: readonly SegmentCameraDurations[]): CompositeExportPlan {
  const plans: CompositeSegmentPlan[] = []
  const progress: number[] = [0]
  let startSeconds = 0
  for (const [segmentIndex, cameras] of segments.entries()) {
    const plan = planCompositeSegment(cameras, startSeconds)
    plans.push(plan)
    for (let frameIndex = 0; frameIndex < plan.frameCount; frameIndex += 1) {
      const value = compositeProgressAfterFrame(segmentIndex, frameIndex, plan.frameCount, segments.length)
      if (value !== null) progress.push(value)
    }
    startSeconds += plan.durationSeconds
  }
  const totalFrames = plans.reduce((sum, plan) => sum + plan.frameCount, 0)
  if (totalFrames === 0) return { error: 'no-playable-media' }
  progress.push(1)
  return { segments: plans, totalFrames, totalDurationSeconds: startSeconds, progress }
}
