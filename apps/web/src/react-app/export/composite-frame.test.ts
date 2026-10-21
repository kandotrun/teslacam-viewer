import { buildCompositeLayout } from './composite-layout'
import {
  compositeFrameStyle,
  createCompositeFrameRenderer,
  type CompositeFrameContext,
  type CompositeLayer,
  type CompositeLayerContext,
} from './composite-frame'

type Recorded = { readonly ops: string[] }

function recordingLayerContext(record: Recorded): CompositeLayerContext {
  return {
    fillStyle: '',
    font: '',
    globalCompositeOperation: 'source-over',
    textAlign: 'left',
    beginPath: () => undefined,
    closePath: () => undefined,
    moveTo: () => undefined,
    lineTo: () => undefined,
    createLinearGradient: () => ({ addColorStop: () => undefined }),
    fill() {
      record.ops.push(`fill ${this.globalCompositeOperation} ${String(this.fillStyle)}`)
    },
    fillRect: (x, y, width, height) => {
      record.ops.push(`fillRect ${x},${y},${width},${height}`)
    },
    roundRect: (x, y, width, height) => {
      record.ops.push(`roundRect ${Math.round(x)},${Math.round(y)},${Math.round(width)},${Math.round(height)}`)
    },
    drawImage: () => {
      record.ops.push('drawImage')
    },
    fillText: (text) => {
      record.ops.push(`fillText ${text}`)
    },
    measureText: (text) => ({ width: text.length * 13 }),
  }
}

function outputContext(record: Recorded): CompositeFrameContext {
  return {
    fillStyle: '',
    fillRect: () => {
      record.ops.push('output fillRect')
    },
    drawImage: () => {
      record.ops.push('output overlay')
    },
  }
}

function renderer(layers: Recorded[]) {
  return createCompositeFrameRenderer({
    layout: buildCompositeLayout(),
    vehicle: { image: document.createElement('img'), width: 360, height: 720 },
    createLayer: (): CompositeLayer => {
      const record: Recorded = { ops: [] }
      layers.push(record)
      return { canvas: document.createElement('canvas'), context: recordingLayerContext(record) }
    },
  })
}

describe('createCompositeFrameRenderer', () => {
  it('draws decoded frames straight into their fitted rectangles and the overlay on top', () => {
    const layers: Recorded[] = []
    const output: Recorded = { ops: [] }
    const drawn: string[] = []

    renderer(layers).drawFrame(outputContext(output), [
      { cameraId: 'front', width: 320, height: 180, draw: (rect) => drawn.push(`front ${[rect.x, rect.y, rect.width, rect.height].map((value) => value.toFixed(2)).join(',')}`) },
    ])

    expect(drawn).toEqual(['front 464.00,45.00,352.00,198.00'])
    expect(output.ops).toEqual(['output fillRect', 'output overlay'])
  })

  it('renders one overlay per slot arrangement and reuses it for every later frame', () => {
    const layers: Recorded[] = []
    const frames = renderer(layers)
    const source = { cameraId: 'front' as const, width: 320, height: 180, draw: () => undefined }

    for (let index = 0; index < 30; index += 1) frames.drawFrame(outputContext({ ops: [] }), [source])
    frames.drawFrame(outputContext({ ops: [] }), [])

    expect(frames.overlayCount()).toBe(2)
    expect(layers).toHaveLength(2)
  })

  it('punches a rounded hole for live video and draws missing cameras as labelled tiles', () => {
    const layers: Recorded[] = []
    const style = compositeFrameStyle()
    renderer(layers).drawFrame(outputContext({ ops: [] }), [
      { cameraId: 'front', width: 320, height: 180, draw: () => undefined },
    ])
    const ops = layers[0]?.ops ?? []

    expect(ops[0]).toBe(`fillRect 0,0,1280,720`)
    expect(ops).toContain('roundRect 464,45,352,198')
    expect(ops).toContain('fill destination-out #000000')
    expect(ops.filter((op) => op === 'fillText 記録なし')).toHaveLength(5)
    expect(ops.filter((op) => op === `fill source-over ${style.missingFill}`)).toHaveLength(5)
    expect(ops).toEqual(expect.arrayContaining(['fillText フロント', 'fillText リア', 'fillText 右リピーター']))
    expect(style).toMatchObject({ background: '#E9EBEE', labelScrim: '#0000008C', labelText: '#FFFFFF', cornerRadius: 12 })
  })
})
