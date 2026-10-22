/**
 * Frame-accurate camera sync: how one follower video is steered towards the source camera.
 *
 * Small drift is removed by playing the follower slightly faster or slower (invisible on muted video);
 * only a large jump seeks it. Drift is measured on the media clocks (`currentTime`), which are precise,
 * plus each video's presentation lead: how far the frame it puts on screen runs ahead of its own clock.
 * Two videos on the same clock can show different frames (a video seeked during playback was measured
 * presenting a whole frame early), so the lead, averaged over many presented frames from
 * `requestVideoFrameCallback`, sets where each follower's clock should be. Steering on the raw presented
 * frames instead chased their frame-to-frame jitter and left more spread on screen.
 */

export const frameSyncTuning = {
  /** A follower further away than this jumps (seeks) to the source. */
  seekBeyondSeconds: 0.3,
  /** At most one seek per follower in this period; a follower that cannot keep up catches up by rate. */
  seekCooldownMs: 1_000,
  /** Nudging starts once the drift exceeds this … */
  nudgeBeyondSeconds: 0.004,
  /** … and stops when it is back within this. */
  settleWithinSeconds: 0.001,
  /** Rate change per second of drift: 20 ms of drift plays 8 % slower or faster. */
  gainPerSecond: 4,
  /** The largest rate change, as a fraction of the chosen playback rate. */
  maxNudge: 0.15,
  /** Rate changes are rounded to 1/200 (0.5 %) so the rate is not rewritten on every frame. */
  nudgeSteps: 200,
  /** Frames ignored after a start, seek or rate change, while the video still shows frames timed before it. */
  leadWarmupFrames: 10,
  /** Weight of each presented frame in the presentation lead (about half a second of frames). */
  leadSmoothing: 0.05,
  /** Presented frames averaged (after the warm-up) before the lead is used. */
  leadFramesBeforeUse: 12,
} as const

/** A frame a video presented: its media time and when it reaches the screen (ms, `performance.now()`). */
export type PresentedFrame = { readonly mediaTime: number; readonly displayedAt: number }

/** A video's clock read at `at` (ms) while playing at `rate`. */
export type ClockReading = { readonly currentTime: number; readonly at: number; readonly rate: number }

/** Media time on screen minus the video's clock at the moment the frame reaches the screen, in seconds. */
export function presentationLead(frame: PresentedFrame, clock: ClockReading): number {
  return frame.mediaTime - (clock.currentTime + ((frame.displayedAt - clock.at) / 1_000) * clock.rate)
}

export type LeadEstimate = { readonly lead: number; readonly frames: number }

export function nextLeadEstimate(estimate: LeadEstimate | undefined, lead: number): LeadEstimate {
  const frames = (estimate?.frames ?? 0) + 1
  // Until the warm-up ends the estimate only holds the latest lead, where the average then starts.
  if (estimate === undefined || frames <= frameSyncTuning.leadWarmupFrames + 1) return { lead, frames }
  return { lead: estimate.lead + frameSyncTuning.leadSmoothing * (lead - estimate.lead), frames }
}

/** The averaged lead once enough frames were seen after the warm-up, else null. */
export function knownLead(estimate: LeadEstimate | undefined): number | null {
  const framesToTrust = frameSyncTuning.leadWarmupFrames + frameSyncTuning.leadFramesBeforeUse
  return estimate !== undefined && estimate.frames >= framesToTrust ? estimate.lead : null
}

export type FollowerSync = {
  readonly nudging: boolean
  /** When the follower was last seeked, in milliseconds. */
  readonly seekedAt: number
}

export const initialFollowerSync: FollowerSync = { nudging: false, seekedAt: Number.NEGATIVE_INFINITY }

export type FollowerInput = {
  /** How far the follower's frames on screen run ahead of the source's, in seconds (clocks plus leads). */
  readonly drift: number
  /** The playback rate the user chose. */
  readonly rate: number
  /** Source `currentTime`, the target of a seek. */
  readonly sourceTime: number
  /** Milliseconds on the `performance.now()` timeline. */
  readonly now: number
}

export type FollowerAction =
  | { readonly kind: 'seek'; readonly mediaTime: number }
  | { readonly kind: 'rate'; readonly playbackRate: number }

export function syncFollower(
  state: FollowerSync,
  { drift, rate, sourceTime, now }: FollowerInput,
): { readonly state: FollowerSync; readonly action: FollowerAction } {
  const tuning = frameSyncTuning
  if (Math.abs(drift) > tuning.seekBeyondSeconds && now - state.seekedAt >= tuning.seekCooldownMs) {
    return { state: { nudging: false, seekedAt: now }, action: { kind: 'seek', mediaTime: sourceTime } }
  }
  const nudging = Math.abs(drift) > (state.nudging ? tuning.settleWithinSeconds : tuning.nudgeBeyondSeconds)
  const nudge = nudging ? Math.min(Math.max(drift * tuning.gainPerSecond, -tuning.maxNudge), tuning.maxNudge) : 0
  const steps = Math.round(nudge * tuning.nudgeSteps)
  return {
    state: { nudging, seekedAt: state.seekedAt },
    action: { kind: 'rate', playbackRate: rate * (1 - steps / tuning.nudgeSteps) },
  }
}
