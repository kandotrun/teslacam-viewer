import { compositeFrameDuration, planCompositeExport, planCompositeSegment } from './composite-plan'

describe('planCompositeExport', () => {
  it('plans one two-second camera as 60 frames with one report per output second', () => {
    expect(planCompositeExport([{ front: 2 }])).toEqual({
      segments: [{ skipped: false, durationSeconds: 2, frameCount: 60, startSeconds: 0, cameraFrameCounts: { front: 60 } }],
      totalFrames: 60,
      totalDurationSeconds: 2,
      progress: [0, 1 / 60, 31 / 60, 1, 1],
    })
  })

  it('concatenates the real durations of five two-second segments', () => {
    const plan = planCompositeExport(Array.from({ length: 5 }, () => ({ front: 2, back: 2 })))

    expect(plan).toMatchObject({ totalFrames: 300, totalDurationSeconds: 10 })
    if ('error' in plan) throw new Error('unexpected plan error')
    expect(plan.segments.map((segment) => segment.startSeconds)).toEqual([0, 2, 4, 6, 8])
  })

  it('skips a segment without decodable cameras and lets progress jump over it', () => {
    const plan = planCompositeExport([{ front: 1 }, { front: null }, { front: 1 }])

    if ('error' in plan) throw new Error('unexpected plan error')
    expect(plan.segments.map((segment) => segment.skipped)).toEqual([false, true, false])
    expect(plan.segments[2]?.startSeconds).toBe(1)
    expect(plan.progress).toContain(1 / 3)
    expect(plan.progress).toContain(2 / 3 + 1 / 90)
  })

  it('counts frames per camera when their lengths differ', () => {
    expect(planCompositeSegment({ front: 20, back: 2 }, 0).cameraFrameCounts).toEqual({ front: 600, back: 60 })
  })

  it('rounds partial frames up', () => {
    expect(planCompositeSegment({ front: 59.97 }, 0).frameCount).toBe(1800)
  })

  it('fails when nothing can be decoded', () => {
    expect(planCompositeExport([{ front: null }, {}])).toEqual({ error: 'no-playable-media' })
  })
})

describe('compositeFrameDuration', () => {
  it('shortens the final frame of a segment to its real end', () => {
    expect(compositeFrameDuration(0, 1)).toBeCloseTo(1 / 30)
    expect(compositeFrameDuration(30, 1.01)).toBeCloseTo(0.01)
  })
})
