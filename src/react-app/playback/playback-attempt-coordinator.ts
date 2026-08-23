import type { CameraId } from '../../teslacam/types'
import {
  attemptMatches,
  resolvePlayback,
  type PlayAttempt,
  type PlaybackResolution,
} from './playback-attempt-state'
import type { PlaybackRuntime } from './playback-runtime'

type PlayCall = {
  readonly cameraId: CameraId
  readonly attempt: PlayAttempt
  readonly promise: Promise<void>
}

type PlaybackStateListener = (resolution: PlaybackResolution) => void

export type PlaybackAttemptCoordinator = {
  readonly refresh: () => void
  readonly invalidate: (cameraId: CameraId) => void
  readonly start: () => void
  readonly stop: () => void
  readonly restartForSourceChange: () => void
  readonly run: (cameraIds: readonly CameraId[]) => Promise<void>
}

function clearAttemptState(runtime: PlaybackRuntime): void {
  runtime.attempts.clear()
  runtime.successfulCameras.clear()
}

function refreshPlaybackState(runtime: PlaybackRuntime, onStateChange: PlaybackStateListener): void {
  const resolution = resolvePlayback(
    runtime.intended,
    [...runtime.registrations.keys()],
    runtime.lifecycles,
    runtime.attempts,
    runtime.successfulCameras,
  )
  if (resolution === 'failed') {
    runtime.intended = false
    runtime.intentGeneration += 1
    clearAttemptState(runtime)
  }
  onStateChange(resolution)
}

function createPlayCalls(runtime: PlaybackRuntime, cameraIds: readonly CameraId[]): PlayCall[] {
  const calls: PlayCall[] = []
  for (const cameraId of cameraIds) {
    const registration = runtime.registrations.get(cameraId)
    if (registration === undefined || runtime.lifecycles.get(cameraId) !== 'ready') continue
    const attempt: PlayAttempt = {
      id: runtime.attemptId + 1,
      element: registration.element,
      intentGeneration: runtime.intentGeneration,
      sourceGeneration: runtime.sourceGeneration,
      streamGeneration: runtime.streamGenerations.get(cameraId) ?? 0,
      status: 'pending',
    }
    runtime.attemptId = attempt.id
    runtime.attempts.set(cameraId, attempt)
    calls.push({ cameraId, attempt, promise: registration.element.play() })
  }
  return calls
}

function settlePlayCalls(runtime: PlaybackRuntime, calls: readonly PlayCall[], results: readonly PromiseSettledResult<void>[]): void {
  for (const [index, call] of calls.entries()) {
    const result = results[index]
    if (result === undefined || !attemptMatches(
      call.attempt,
      runtime.attempts.get(call.cameraId),
      runtime.registrations.get(call.cameraId)?.element,
      runtime.intentGeneration,
      runtime.sourceGeneration,
      runtime.streamGenerations.get(call.cameraId) ?? 0,
    )) continue
    runtime.attempts.set(call.cameraId, { ...call.attempt, status: result.status })
    if (result.status === 'fulfilled') runtime.successfulCameras.add(call.cameraId)
    else runtime.successfulCameras.delete(call.cameraId)
  }
}

export function createPlaybackAttemptCoordinator(
  runtime: PlaybackRuntime,
  onStateChange: PlaybackStateListener,
): PlaybackAttemptCoordinator {
  const refresh = (): void => refreshPlaybackState(runtime, onStateChange)
  const stop = (): void => {
    runtime.intended = false
    runtime.intentGeneration += 1
    clearAttemptState(runtime)
  }

  return {
    refresh,
    invalidate: (cameraId): void => {
      runtime.streamGenerations.set(cameraId, (runtime.streamGenerations.get(cameraId) ?? 0) + 1)
      runtime.attempts.delete(cameraId)
      runtime.successfulCameras.delete(cameraId)
    },
    start: (): void => {
      runtime.intended = true
      runtime.intentGeneration += 1
      clearAttemptState(runtime)
    },
    stop,
    restartForSourceChange: (): void => {
      runtime.sourceGeneration += 1
      clearAttemptState(runtime)
    },
    run: async (cameraIds): Promise<void> => {
      const calls = createPlayCalls(runtime, cameraIds)
      if (calls.length === 0) {
        refresh()
        return
      }
      refresh()
      const results = await Promise.allSettled(calls.map((call) => call.promise))
      settlePlayCalls(runtime, calls, results)
      refresh()
    },
  }
}
