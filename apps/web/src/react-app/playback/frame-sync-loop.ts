import {
  initialFollowerSync,
  knownLead,
  nextLeadEstimate,
  presentationLead,
  syncFollower,
  type FollowerSync,
  type LeadEstimate,
} from './frame-sync'
import type { PlaybackRuntime } from './playback-runtime'

export type FrameSyncLoop = {
  /** Follows the frames this element presents; each frame of the elected source steers the others. */
  readonly watch: (element: HTMLVideoElement) => void
  readonly unwatch: (element: HTMLVideoElement) => void
  /** Forgets nudges and leads and restores the chosen rate on every camera (after a seek, a rate change or a pause). */
  readonly reset: () => void
}

function canFollow(element: HTMLVideoElement): boolean {
  return !element.paused && !element.seeking && !element.ended && element.readyState >= HTMLMediaElement.HAVE_FUTURE_DATA
}

/**
 * Keeps the registered cameras on the source camera's frames. Every watched video asks for a callback
 * on each frame it presents (`requestVideoFrameCallback`), which updates its presentation lead; each
 * source frame compares every follower's clock and lead with the source's and nudges or seeks it
 * (see `syncFollower`).
 */
export function createFrameSyncLoop(runtime: PlaybackRuntime): FrameSyncLoop {
  const handles = new Map<HTMLVideoElement, number>()
  const followers = new Map<HTMLVideoElement, FollowerSync>()
  const leads = new Map<HTMLVideoElement, LeadEstimate>()
  let lastSource: HTMLVideoElement | null = null

  const sourceElement = (): HTMLVideoElement | undefined => (
    runtime.sourceCamera === null ? undefined : runtime.registrations.get(runtime.sourceCamera)?.element
  )

  const steerFollowers = (source: HTMLVideoElement, now: number): void => {
    if (source !== lastSource) {
      followers.clear()
      lastSource = source
    }
    if (source.playbackRate !== runtime.rate) source.playbackRate = runtime.rate
    const sourceTime = source.currentTime
    const sourceLead = knownLead(leads.get(source))
    for (const [cameraId, { element }] of runtime.registrations) {
      if (element === source || !runtime.readyCameras.has(cameraId) || !canFollow(element)) continue
      const lead = knownLead(leads.get(element))
      const { state, action } = syncFollower(followers.get(element) ?? initialFollowerSync, {
        drift: element.currentTime - sourceTime + (lead === null || sourceLead === null ? 0 : lead - sourceLead),
        rate: runtime.rate,
        sourceTime,
        now,
      })
      followers.set(element, state)
      if (action.kind === 'seek') {
        leads.delete(element)
        element.playbackRate = runtime.rate
        element.currentTime = action.mediaTime
      } else if (element.playbackRate !== action.playbackRate) {
        element.playbackRate = action.playbackRate
      }
    }
  }

  const watch = (element: HTMLVideoElement): void => {
    // Browsers without frame callbacks keep playing every camera, just without steering.
    if (handles.has(element) || !('requestVideoFrameCallback' in element)) return
    const onFrame = (now: number, metadata: VideoFrameCallbackMetadata): void => {
      if (!handles.has(element)) return
      handles.set(element, element.requestVideoFrameCallback(onFrame))
      if (element.paused || element.seeking) return
      const frame = { mediaTime: metadata.mediaTime, displayedAt: metadata.expectedDisplayTime }
      const clock = { currentTime: element.currentTime, at: now, rate: element.playbackRate }
      leads.set(element, nextLeadEstimate(leads.get(element), presentationLead(frame, clock)))
      if (element === sourceElement()) steerFollowers(element, now)
    }
    handles.set(element, element.requestVideoFrameCallback(onFrame))
  }

  return {
    watch,
    unwatch: (element) => {
      const handle = handles.get(element)
      if (handle !== undefined) element.cancelVideoFrameCallback(handle)
      handles.delete(element)
      followers.delete(element)
      leads.delete(element)
    },
    reset: () => {
      followers.clear()
      leads.clear()
      for (const { element } of runtime.registrations.values()) element.playbackRate = runtime.rate
    },
  }
}
