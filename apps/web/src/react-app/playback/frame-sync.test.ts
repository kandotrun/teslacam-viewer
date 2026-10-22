import {
  frameSyncTuning,
  initialFollowerSync,
  knownLead,
  nextLeadEstimate,
  presentationLead,
  syncFollower,
  type FollowerInput,
} from './frame-sync'

const steady: FollowerInput = { drift: 0, rate: 1, sourceTime: 10, now: 5_000 }

function steer(input: Partial<FollowerInput>, state = initialFollowerSync): ReturnType<typeof syncFollower> {
  return syncFollower(state, { ...steady, ...input })
}

describe('syncFollower', () => {
  it('keeps a follower on the source clock at the chosen rate', () => {
    expect(steer({ rate: 2 }).action).toEqual({ kind: 'rate', playbackRate: 2 })
  })

  it('seeks a follower further than the seek threshold to the source', () => {
    const result = steer({ drift: -0.31 })
    expect(result.action).toEqual({ kind: 'seek', mediaTime: 10 })
    expect(result.state).toEqual({ nudging: false, seekedAt: 5_000 })
  })

  it('catches up by rate instead of seeking again within the seek cooldown', () => {
    const seeked = steer({ drift: -0.5 }).state
    const again = steer({ drift: -0.5, now: 5_000 + frameSyncTuning.seekCooldownMs - 1 }, seeked)
    expect(again.action).toEqual({ kind: 'rate', playbackRate: 1 + frameSyncTuning.maxNudge })
    expect(steer({ drift: -0.5, now: 5_000 + frameSyncTuning.seekCooldownMs }, seeked).action.kind).toBe('seek')
  })

  it('slows a follower that runs ahead and speeds up one that lags, in proportion and capped', () => {
    expect(steer({ drift: 0.02 }).action).toEqual({ kind: 'rate', playbackRate: 1 - 0.08 })
    expect(steer({ drift: -0.02 }).action).toEqual({ kind: 'rate', playbackRate: 1 + 0.08 })
    expect(steer({ drift: 0.25 }).action).toEqual({ kind: 'rate', playbackRate: 1 - frameSyncTuning.maxNudge })
  })

  it('nudges relative to the chosen playback rate', () => {
    expect(steer({ rate: 2, drift: 0.02 }).action).toEqual({ kind: 'rate', playbackRate: 2 * (1 - 0.08) })
  })

  it('starts nudging beyond the nudge threshold and keeps nudging until the drift settles', () => {
    const below = steer({ drift: frameSyncTuning.nudgeBeyondSeconds * 0.9 })
    expect(below).toEqual({ state: { nudging: false, seekedAt: Number.NEGATIVE_INFINITY }, action: { kind: 'rate', playbackRate: 1 } })

    const nudging = steer({ drift: frameSyncTuning.nudgeBeyondSeconds * 1.5 }).state
    const halfway = steer({ drift: frameSyncTuning.nudgeBeyondSeconds * 0.5 }, nudging)
    expect(halfway.state.nudging).toBe(true)
    expect(halfway.action).not.toEqual({ kind: 'rate', playbackRate: 1 })

    const settled = steer({ drift: frameSyncTuning.settleWithinSeconds * 0.5 }, halfway.state)
    expect(settled).toEqual({ state: { nudging: false, seekedAt: Number.NEGATIVE_INFINITY }, action: { kind: 'rate', playbackRate: 1 } })
  })

  it('rounds nudges to steps of 0.5 % so the rate is not rewritten on every frame', () => {
    expect(steer({ drift: 0.0051 }).action).toEqual(steer({ drift: 0.0049 }).action)
  })
})

describe('presentation lead', () => {
  it('measures how far the frame on screen is ahead of the clock when it reaches the screen', () => {
    // The clock reads 10 s at 1 000 ms; the frame due on screen at 1 016 ms shows 10.05 s at 2×.
    expect(presentationLead({ mediaTime: 10.05, displayedAt: 1_016 }, { currentTime: 10, at: 1_000, rate: 2 })).toBeCloseTo(0.018)
  })

  it('skips the frames right after a start, seek or rate change, then smooths and trusts the lead', () => {
    const framesToTrust = frameSyncTuning.leadWarmupFrames + frameSyncTuning.leadFramesBeforeUse
    let estimate = nextLeadEstimate(undefined, 0.5)
    for (let frame = 1; frame < frameSyncTuning.leadWarmupFrames; frame += 1) estimate = nextLeadEstimate(estimate, 0.5)
    for (let frame = frameSyncTuning.leadWarmupFrames; frame < framesToTrust; frame += 1) {
      expect(knownLead(estimate)).toBeNull()
      estimate = nextLeadEstimate(estimate, 0.03)
    }
    expect(knownLead(estimate)).toBeCloseTo(0.03)
    expect(knownLead(nextLeadEstimate(estimate, 0.3))).toBeCloseTo(0.03 + frameSyncTuning.leadSmoothing * 0.27)
  })
})
