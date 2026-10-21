/**
 * Draws export frames. Decoded camera frames go straight into the output canvas at their contain-fitted
 * rectangles; one static overlay (floor with rounded holes, wedges, car, missing tiles and name pills) is
 * rendered once per slot arrangement and drawn on top of every frame.
 */
import type { CameraId } from '../../teslacam/types'
import { colorToken, compositeLayoutContract, fontStack } from '../design/contract'
import {
  compositeLabelGeometry,
  compositeOverlayKey,
  compositeSlotFrames,
  containRect,
  type CompositeLayout,
  type CompositeRect,
  type CompositeSlotFrame,
} from './composite-layout'

/** The 2D canvas operations the overlay needs (a subset, so tests can record calls). */
export type CompositeLayerContext = Pick<
  CanvasRenderingContext2D,
  | 'beginPath'
  | 'closePath'
  | 'createLinearGradient'
  | 'drawImage'
  | 'fill'
  | 'fillRect'
  | 'fillText'
  | 'lineTo'
  | 'moveTo'
  | 'roundRect'
> & {
  measureText: (text: string) => { readonly width: number }
  fillStyle: CanvasRenderingContext2D['fillStyle']
  font: string
  globalCompositeOperation: GlobalCompositeOperation
  textAlign: CanvasTextAlign
}

/** The output canvas: frames are drawn into it, then the overlay layer on top. */
export type CompositeFrameContext = Pick<CanvasRenderingContext2D, 'drawImage' | 'fillRect'> & {
  fillStyle: CanvasRenderingContext2D['fillStyle']
}

export type CompositeLayer = {
  readonly canvas: CanvasImageSource
  readonly context: CompositeLayerContext
}

export type CompositeFrameSource = {
  readonly cameraId: CameraId
  readonly width: number
  readonly height: number
  /** Draws the current camera frame scaled into `rect` of the output canvas. */
  readonly draw: (rect: CompositeRect) => void
}

export type CompositeVehicleImage = {
  readonly image: CanvasImageSource
  readonly width: number
  readonly height: number
}

export type CompositeFrameStyle = {
  readonly background: string
  readonly wedge: string
  readonly wedgeFaded: string
  readonly missingFill: string
  readonly missingText: string
  readonly missingMessage: string
  readonly labelScrim: string
  readonly labelText: string
  readonly font: string
  readonly cornerRadius: number
}

export type CompositeFrameRenderer = {
  readonly drawFrame: (context: CompositeFrameContext, sources: readonly CompositeFrameSource[]) => void
  /** Number of distinct overlays rendered so far. */
  readonly overlayCount: () => number
}

function transparent(color: string): string {
  return `${color.slice(0, 7)}00`
}

export function compositeFrameStyle(): CompositeFrameStyle {
  const contract = compositeLayoutContract
  const wedge = colorToken(contract.wedges.color)
  return {
    background: colorToken(contract.background),
    wedge,
    wedgeFaded: transparent(wedge),
    missingFill: colorToken(contract.missing.fill),
    missingText: colorToken(contract.missing.text),
    missingMessage: contract.missing.message,
    labelScrim: colorToken(contract.label.scrim),
    labelText: colorToken(contract.label.text),
    font: `${contract.label.fontWeight} ${contract.label.fontSize}px ${fontStack('ui')}`,
    cornerRadius: contract.videoCornerRadius,
  }
}

function fillRoundRect(context: CompositeLayerContext, rect: CompositeRect, radius: number): void {
  context.beginPath()
  context.roundRect(rect.x, rect.y, rect.width, rect.height, radius)
  context.fill()
}

function drawWedges(context: CompositeLayerContext, slots: readonly CompositeSlotFrame[], style: CompositeFrameStyle): void {
  for (const { wedge } of slots) {
    const gradient = context.createLinearGradient(wedge.mount.x, wedge.mount.y, wedge.edgeMidpoint.x, wedge.edgeMidpoint.y)
    gradient.addColorStop(0, style.wedge)
    gradient.addColorStop(1, style.wedgeFaded)
    const [first, second] = wedge.corners
    context.fillStyle = gradient
    context.beginPath()
    context.moveTo(wedge.mount.x, wedge.mount.y)
    context.lineTo(first.x, first.y)
    context.lineTo(second.x, second.y)
    context.closePath()
    context.fill()
  }
}

export function drawCompositeOverlay(
  layer: CompositeLayerContext,
  layout: CompositeLayout,
  slots: readonly CompositeSlotFrame[],
  vehicle: CompositeVehicleImage,
  style: CompositeFrameStyle,
): void {
  layer.globalCompositeOperation = 'source-over'
  layer.fillStyle = style.background
  layer.fillRect(0, 0, layout.width, layout.height)
  drawWedges(layer, slots, style)
  const car = containRect(vehicle, layout.vehicle)
  layer.drawImage(vehicle.image, car.x, car.y, car.width, car.height)

  layer.globalCompositeOperation = 'destination-out'
  layer.fillStyle = '#000000'
  for (const slot of slots) if (slot.playable) fillRoundRect(layer, slot.videoRect, style.cornerRadius)
  layer.globalCompositeOperation = 'source-over'

  layer.font = style.font
  for (const slot of slots) {
    if (slot.playable) continue
    layer.fillStyle = style.missingFill
    fillRoundRect(layer, slot.videoRect, style.cornerRadius)
    layer.fillStyle = style.missingText
    layer.textAlign = 'center'
    layer.fillText(style.missingMessage, slot.videoRect.x + slot.videoRect.width / 2, slot.videoRect.y + slot.videoRect.height / 2 + 4)
  }
  layer.textAlign = 'left'
  for (const slot of slots) {
    const label = compositeLabelGeometry(slot.label, slot.videoRect, layer.measureText(slot.label).width)
    layer.fillStyle = style.labelScrim
    fillRoundRect(layer, label.pill, label.radius)
    layer.fillStyle = style.labelText
    layer.fillText(label.text, label.textX, label.textY)
  }
}

export function createCompositeFrameRenderer(options: {
  readonly layout: CompositeLayout
  readonly vehicle: CompositeVehicleImage
  readonly createLayer: () => CompositeLayer
  readonly style?: CompositeFrameStyle
}): CompositeFrameRenderer {
  const style = options.style ?? compositeFrameStyle()
  const overlays = new Map<string, CompositeLayer>()
  let referenceSize: { readonly width: number; readonly height: number } | null = null

  const overlayFor = (slots: readonly CompositeSlotFrame[]): CompositeLayer => {
    const key = compositeOverlayKey(slots)
    const cached = overlays.get(key)
    if (cached !== undefined) return cached
    const layer = options.createLayer()
    drawCompositeOverlay(layer.context, options.layout, slots, options.vehicle, style)
    overlays.set(key, layer)
    return layer
  }

  return {
    drawFrame: (context, sources) => {
      const sizes: Partial<Record<CameraId, { readonly width: number; readonly height: number }>> = {}
      for (const source of sources) {
        if (source.width <= 0 || source.height <= 0) continue
        sizes[source.cameraId] = { width: source.width, height: source.height }
        referenceSize ??= { width: source.width, height: source.height }
      }
      const slots = compositeSlotFrames(options.layout, sizes, referenceSize)
      context.fillStyle = style.background
      context.fillRect(0, 0, options.layout.width, options.layout.height)
      for (const source of sources) {
        const slot = slots.find(({ cameraId }) => cameraId === source.cameraId)
        if (slot?.playable === true) source.draw(slot.videoRect)
      }
      context.drawImage(overlayFor(slots).canvas, 0, 0)
    },
    overlayCount: () => overlays.size,
  }
}
