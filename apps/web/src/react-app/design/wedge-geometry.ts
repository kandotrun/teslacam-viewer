/**
 * Field-of-view wedges: a soft triangle from a camera's mount point on the car to the two corners of its
 * window that face the car. Shared by the stage (SVG) and the composite export (canvas).
 */
import type { MountPoint } from './contract'

export type Point = { readonly x: number; readonly y: number }

export type Rect = {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
}

export type Wedge = {
  readonly mount: Point
  readonly corners: readonly [Point, Point]
  /** Where the gradient ends (the middle of the facing edge). */
  readonly edgeMidpoint: Point
}

const verticalEdgeInset = 10
const horizontalEdgeInset = 14

/** Mount point in the same coordinates as `carBox`, from fractions of the car image box. */
export function mountPointIn(carBox: Rect, [fractionX, fractionY]: MountPoint): Point {
  return { x: carBox.x + carBox.width * fractionX, y: carBox.y + carBox.height * fractionY }
}

export function fieldOfViewWedge(mount: Point, window: Rect): Wedge {
  const centerX = window.x + window.width / 2
  const centerY = window.y + window.height / 2
  const right = window.x + window.width
  const bottom = window.y + window.height
  const horizontal = Math.abs(centerX - mount.x) > Math.abs(centerY - mount.y)
  let corners: readonly [Point, Point]
  if (horizontal) {
    const edgeX = centerX < mount.x ? right : window.x
    corners = [{ x: edgeX, y: window.y + verticalEdgeInset }, { x: edgeX, y: bottom - verticalEdgeInset }]
  } else {
    const edgeY = centerY < mount.y ? bottom : window.y
    corners = [{ x: window.x + horizontalEdgeInset, y: edgeY }, { x: right - horizontalEdgeInset, y: edgeY }]
  }
  const [first, second] = corners
  return {
    mount,
    corners,
    edgeMidpoint: { x: (first.x + second.x) / 2, y: (first.y + second.y) / 2 },
  }
}

export function wedgePolygonPoints(wedge: Wedge): string {
  const [first, second] = wedge.corners
  return [wedge.mount, first, second].map(({ x, y }) => `${round(x)},${round(y)}`).join(' ')
}

function round(value: number): number {
  return Math.round(value * 100) / 100
}
