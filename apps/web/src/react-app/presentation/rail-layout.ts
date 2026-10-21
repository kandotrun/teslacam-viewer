/**
 * Horizontal placement of the day rail's interactive items (single markers and cluster pills) so that
 * they never overlap and stay inside the track. Pure.
 */

export type RailItemRequest = {
  /** Desired centre in pixels from the track's left edge. */
  readonly center: number
  readonly width: number
}

const itemGap = 2

/** Left edges (px) for items given in ascending centre order. */
export function layoutRailItems(items: readonly RailItemRequest[], trackWidth: number): readonly number[] {
  const lefts = items.map(({ center, width }) => Math.min(Math.max(center - width / 2, 0), Math.max(0, trackWidth - width)))
  for (let index = 1; index < items.length; index += 1) {
    const previousLeft = lefts[index - 1] ?? 0
    const previousWidth = items[index - 1]?.width ?? 0
    lefts[index] = Math.max(lefts[index] ?? 0, previousLeft + previousWidth + itemGap)
  }
  for (let index = items.length - 1; index >= 0; index -= 1) {
    const width = items[index]?.width ?? 0
    const nextLeft = index === items.length - 1 ? trackWidth + itemGap : (lefts[index + 1] ?? trackWidth)
    lefts[index] = Math.max(0, Math.min(lefts[index] ?? 0, nextLeft - itemGap - width))
  }
  return lefts
}

/** Whether full cluster pills (time range and count) fit; otherwise pills show the count only. */
export function railPillsFit(itemWidths: readonly number[], trackWidth: number): boolean {
  const total = itemWidths.reduce((sum, width) => sum + width, 0) + Math.max(0, itemWidths.length - 1) * itemGap
  return total <= trackWidth
}

/** `20:35` from `20:35:30`. */
export function shortTime(timeLabel: string): string {
  return timeLabel.slice(0, 5)
}

/** `20:35–20:45` (en dash). */
export function clusterRange(cluster: { readonly startTimeLabel: string; readonly endTimeLabel: string }): string {
  return `${shortTime(cluster.startTimeLabel)}–${shortTime(cluster.endTimeLabel)}`
}
