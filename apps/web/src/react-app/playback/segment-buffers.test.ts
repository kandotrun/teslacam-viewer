import { segmentBuffers } from './segment-buffers'

describe('segmentBuffers', () => {
  it('plays even segments in the first video and odd ones in the second, preloading the next segment', () => {
    expect(segmentBuffers(0, 3)).toEqual({ activeSlot: 0, spareSegmentIndex: 1 })
    expect(segmentBuffers(1, 3)).toEqual({ activeSlot: 1, spareSegmentIndex: 2 })
  })

  it('keeps the previous segment in the spare video at the last segment', () => {
    expect(segmentBuffers(2, 3)).toEqual({ activeSlot: 0, spareSegmentIndex: 1 })
  })

  it('has nothing to preload for a single segment', () => {
    expect(segmentBuffers(0, 1)).toEqual({ activeSlot: 0, spareSegmentIndex: null })
  })

  it('hands the preloaded video over when playback moves on, and keeps it when it moves back', () => {
    const playing = segmentBuffers(4, 9)
    const next = segmentBuffers(5, 9)
    expect(next.activeSlot).not.toBe(playing.activeSlot)
    expect(playing.spareSegmentIndex).toBe(5)
    expect(segmentBuffers(4, 9).spareSegmentIndex).toBe(5)
    expect(segmentBuffers(3, 9)).toEqual({ activeSlot: next.activeSlot, spareSegmentIndex: 4 })
  })
})
