/**
 * Geometry of the 1280 × 720 export frame (DESIGN.md "Export frame", `composite-layout.json`).
 * Everything here is pure so that the web and Mac exporters can be checked against the same numbers.
 */
import type { CameraId } from '../../teslacam/types'
import { cameraMounts, compositeLayoutContract } from '../design/contract'
import { fieldOfViewWedge, mountPointIn, type Rect, type Wedge } from '../design/wedge-geometry'

export type CompositeRect = Rect

export type CompositeCameraSlot = {
  readonly cameraId: CameraId
  readonly label: string
  readonly frame: CompositeRect
}

export type CompositeLayout = {
  readonly width: number
  readonly height: number
  readonly fps: number
  readonly vehicle: CompositeRect
  readonly cameras: readonly CompositeCameraSlot[]
}

type Size = {
  readonly width: number
  readonly height: number
}

/** Draw order of the slots (parity spec §1.1). */
export const compositeSlotOrder = [
  'front',
  'left_pillar',
  'right_pillar',
  'left_repeater',
  'back',
  'right_repeater',
] as const satisfies readonly CameraId[]

export function buildCompositeLayout(): CompositeLayout {
  const contract = compositeLayoutContract
  return {
    width: contract.width,
    height: contract.height,
    fps: contract.fps,
    vehicle: contract.vehicle,
    cameras: compositeSlotOrder.map((cameraId) => ({
      cameraId,
      label: contract.labels[cameraId],
      frame: contract.slots[cameraId],
    })),
  }
}

export function containRect(source: Size, destination: CompositeRect): CompositeRect {
  if (source.width <= 0 || source.height <= 0) return destination
  const scale = Math.min(destination.width / source.width, destination.height / source.height)
  const width = source.width * scale
  const height = source.height * scale
  return {
    x: destination.x + (destination.width - width) / 2,
    y: destination.y + (destination.height - height) / 2,
    width,
    height,
  }
}

export type CompositeLabelGeometry = {
  readonly text: string
  readonly pill: CompositeRect
  readonly radius: number
  /** Alphabetic baseline origin of the text. */
  readonly textX: number
  readonly textY: number
}

/** Name pill at the bottom-left of the visible video rectangle. */
export function compositeLabelGeometry(text: string, videoRect: CompositeRect, textAdvanceWidth: number): CompositeLabelGeometry {
  const { inset, height, paddingInline, fontSize } = compositeLayoutContract.label
  const width = Math.ceil(textAdvanceWidth) + paddingInline * 2
  const pill = { x: videoRect.x + inset, y: videoRect.y + videoRect.height - inset - height, width, height }
  return {
    text,
    pill,
    radius: height / 2,
    textX: pill.x + paddingInline,
    textY: pill.y + height / 2 + fontSize * 0.36,
  }
}

export type CompositeSlotFrame = {
  readonly cameraId: CameraId
  readonly label: string
  /** The video rectangle (contain-fitted); a missing camera keeps the same shape, drawn as a tile. */
  readonly videoRect: CompositeRect
  readonly playable: boolean
  readonly wedge: Wedge
}

/**
 * Per-slot geometry for one output frame. `sources` holds the decoded size of each camera that has a
 * frame now; `referenceSize` shapes the tiles of cameras without one (normally the size of the others).
 */
export function compositeSlotFrames(
  layout: CompositeLayout,
  sources: Readonly<Partial<Record<CameraId, Size>>>,
  referenceSize: Size | null,
): readonly CompositeSlotFrame[] {
  return layout.cameras.map(({ cameraId, label, frame }) => {
    const source = sources[cameraId]
    const playable = source !== undefined && source.width > 0 && source.height > 0
    const videoRect = containRect(playable ? source : (referenceSize ?? frame), frame)
    const mount = mountPointIn(layout.vehicle, cameraMounts[cameraId])
    return { cameraId, label, videoRect, playable, wedge: fieldOfViewWedge(mount, videoRect) }
  })
}

/** Identifies an overlay: overlays only change when a slot's rectangle or availability changes. */
export function compositeOverlayKey(slots: readonly CompositeSlotFrame[]): string {
  return slots
    .map(({ cameraId, playable, videoRect }) => {
      const rect = [videoRect.x, videoRect.y, videoRect.width, videoRect.height].map((value) => value.toFixed(2)).join(',')
      return `${cameraId}:${playable ? 'video' : 'missing'}:${rect}`
    })
    .join('|')
}
