export type BufferSlot = 0 | 1

/** Which of a camera window's two videos plays the current segment, and which segment the other holds. */
export type SegmentBuffers = {
  readonly activeSlot: BufferSlot
  readonly spareSegmentIndex: number | null
}

/**
 * Segment `k` always plays in video `k % 2`, so the other video can preload segment `k + 1` and take
 * over at the boundary without loading anything. At the last segment the other video keeps `k - 1`.
 */
export function segmentBuffers(segmentIndex: number, segmentCount: number): SegmentBuffers {
  const activeSlot: BufferSlot = segmentIndex % 2 === 0 ? 0 : 1
  if (segmentIndex + 1 < segmentCount) return { activeSlot, spareSegmentIndex: segmentIndex + 1 }
  return { activeSlot, spareSegmentIndex: segmentIndex > 0 ? segmentIndex - 1 : null }
}
