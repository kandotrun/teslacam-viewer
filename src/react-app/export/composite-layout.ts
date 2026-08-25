import type { CameraId } from '../../teslacam/types'
import { cameraLabels } from '../event-summary'
import type { CompositeCanvasVisualTokens } from './composite-visual-tokens'

export type CompositeRect = {
  readonly x: number
  readonly y: number
  readonly width: number
  readonly height: number
}

export type CompositeCameraSlot = {
  readonly cameraId: CameraId
  readonly label: string
  readonly frame: CompositeRect
}

export type CompositeLayout = {
  readonly width: number
  readonly height: number
  readonly vehicle: CompositeRect
  readonly cameras: readonly CompositeCameraSlot[]
}

export type CompositeDrawable = {
  readonly cameraId: CameraId
  readonly source: CanvasImageSource
  readonly width: number
  readonly height: number
  readonly playable: boolean
}

type Size = {
  readonly width: number
  readonly height: number
}

const cameraFrames = [
  ['front', { x: 400, y: 12, width: 480, height: 216 }],
  ['left_pillar', { x: 12, y: 180, width: 384, height: 216 }],
  ['right_pillar', { x: 884, y: 180, width: 384, height: 216 }],
  ['left_repeater', { x: 12, y: 492, width: 384, height: 216 }],
  ['back', { x: 400, y: 492, width: 480, height: 216 }],
  ['right_repeater', { x: 884, y: 492, width: 384, height: 216 }],
] as const satisfies readonly (readonly [CameraId, CompositeRect])[]

export function buildCompositeLayout(): CompositeLayout {
  return {
    width: 1280,
    height: 720,
    vehicle: { x: 496, y: 260, width: 288, height: 184 },
    cameras: cameraFrames.map(([cameraId, frame]) => ({
      cameraId,
      label: cameraLabels[cameraId],
      frame,
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

export function drawCompositeFrame(
  context: CanvasRenderingContext2D,
  layout: CompositeLayout,
  clips: readonly CompositeDrawable[],
  vehicleImage: HTMLImageElement,
  visualTokens: CompositeCanvasVisualTokens,
): void {
  context.fillStyle = visualTokens.canvas
  context.fillRect(0, 0, layout.width, layout.height)
  for (const slot of layout.cameras) {
    const { x, y, width, height } = slot.frame
    context.fillStyle = visualTokens.cameraSurface
    context.fillRect(x, y, width, height)
    const clip = clips.find(({ cameraId }) => cameraId === slot.cameraId)
    const playable = clip?.playable === true
    if (playable && clip !== undefined) {
      const destination = containRect({ width: clip.width, height: clip.height }, slot.frame)
      context.drawImage(clip.source, destination.x, destination.y, destination.width, destination.height)
    }
    const label = playable ? slot.label : `${slot.label}・記録なし`
    context.font = visualTokens.labelFont
    const labelX = x + visualTokens.labelInset
    const labelY = y + visualTokens.labelInset
    const labelWidth = Math.ceil(context.measureText(label).width) + visualTokens.labelPaddingInline * 2
    context.fillStyle = visualTokens.labelScrim
    context.fillRect(labelX, labelY, labelWidth, visualTokens.labelHeight)
    context.fillStyle = visualTokens.labelText
    context.fillText(label, labelX + visualTokens.labelPaddingInline, labelY + visualTokens.labelBaseline)
  }
  const vehicle = containRect({ width: vehicleImage.naturalWidth, height: vehicleImage.naturalHeight }, layout.vehicle)
  context.drawImage(vehicleImage, vehicle.x, vehicle.y, vehicle.width, vehicle.height)
}
