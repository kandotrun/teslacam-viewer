import { useId, useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from 'react'

import { cameraIds, type CameraId } from '../../../teslacam/types'
import { cameraMounts, colorToken } from '../../design/contract'
import { fieldOfViewWedge, mountPointIn, wedgePolygonPoints, type Rect, type Wedge } from '../../design/wedge-geometry'
import type { StageMode } from '../../presentation/stage-layout'

type CameraWedge = Wedge & { readonly cameraId: CameraId }

type StageFrameProps = {
  readonly mode: StageMode | 'empty'
  /** Camera whose wedge and mount dot are drawn in `drive`; null draws every wedge neutral. */
  readonly activeCamera: CameraId | null
  readonly reveal?: boolean
  /** Camera windows: elements with the `camera-window` class and a `data-camera` id. */
  readonly children: ReactNode
  readonly extra?: ReactNode
}

function relativeRect(element: Element, origin: DOMRect): Rect {
  const bounds = element.getBoundingClientRect()
  return { x: bounds.left - origin.left, y: bounds.top - origin.top, width: bounds.width, height: bounds.height }
}

function transparent(color: string): string {
  return `${color.slice(0, 7)}00`
}

/** Measures the windows and the car and returns one wedge per camera (stage coordinates). */
function useWedges(
  stageRef: RefObject<HTMLElement | null>,
  carRef: RefObject<HTMLElement | null>,
  enabled: boolean,
): { readonly wedges: readonly CameraWedge[]; readonly size: { readonly width: number; readonly height: number } } {
  const [measured, setMeasured] = useState<{ readonly wedges: readonly CameraWedge[]; readonly size: { readonly width: number; readonly height: number } }>({
    wedges: [],
    size: { width: 0, height: 0 },
  })

  useLayoutEffect(() => {
    const stage = stageRef.current
    const car = carRef.current
    if (!enabled || stage === null || car === null) {
      setMeasured((current) => (current.wedges.length === 0 ? current : { wedges: [], size: current.size }))
      return undefined
    }
    const measure = (): void => {
      const origin = stage.getBoundingClientRect()
      const carBox = relativeRect(car, origin)
      if (carBox.width <= 0 || carBox.height <= 0) return
      const windows = new Map<string, Element>()
      for (const element of stage.querySelectorAll('.camera-window[data-camera]')) {
        windows.set(element.getAttribute('data-camera') ?? '', element)
      }
      const wedges = cameraIds.flatMap((cameraId): CameraWedge[] => {
        const element = windows.get(cameraId)
        if (element === undefined) return []
        const window = relativeRect(element, origin)
        if (window.width <= 0 || window.height <= 0) return []
        return [{ cameraId, ...fieldOfViewWedge(mountPointIn(carBox, cameraMounts[cameraId]), window) }]
      })
      setMeasured({ wedges, size: { width: origin.width, height: origin.height } })
    }
    measure()
    if (typeof ResizeObserver === 'undefined') return undefined
    const observer = new ResizeObserver(measure)
    observer.observe(stage)
    observer.observe(car)
    return () => observer.disconnect()
  }, [carRef, enabled, stageRef])

  return measured
}

/**
 * The garage floor stage: the car in the middle row with soft field-of-view wedges drawn under the
 * camera windows (recomputed whenever the stage resizes).
 */
export function StageFrame({ mode, activeCamera, reveal = false, children, extra }: StageFrameProps) {
  const stageRef = useRef<HTMLElement>(null)
  const carRef = useRef<HTMLDivElement>(null)
  const gradientPrefix = useId()
  const showCar = mode === 'spatial' || mode === 'empty'
  const { wedges, size } = useWedges(stageRef, carRef, showCar)
  const neutral = colorToken('wedge')
  const active = colorToken('wedge-active')

  return (
    <main aria-label="6台のカメラ" className="stage" data-mode={mode} data-reveal={reveal} ref={stageRef}>
      {showCar && wedges.length > 0 && (
        <svg aria-hidden="true" className="stage__wedges" viewBox={`0 0 ${size.width} ${size.height}`}>
          <defs>
            {wedges.map((wedge) => {
              const color = wedge.cameraId === activeCamera ? active : neutral
              return (
                <linearGradient
                  gradientUnits="userSpaceOnUse"
                  id={`${gradientPrefix}-${wedge.cameraId}`}
                  key={wedge.cameraId}
                  x1={wedge.mount.x}
                  x2={wedge.edgeMidpoint.x}
                  y1={wedge.mount.y}
                  y2={wedge.edgeMidpoint.y}
                >
                  <stop offset="0" stopColor={color} />
                  <stop offset="1" stopColor={transparent(color)} />
                </linearGradient>
              )
            })}
          </defs>
          {wedges.map((wedge) => (
            <polygon
              data-active={wedge.cameraId === activeCamera}
              data-camera={wedge.cameraId}
              fill={`url(#${gradientPrefix}-${wedge.cameraId})`}
              key={wedge.cameraId}
              points={wedgePolygonPoints(wedge)}
            />
          ))}
        </svg>
      )}
      {showCar && (
        <div className="stage__car-slot">
          <div className="stage__car" ref={carRef}>
            <img alt="" draggable={false} height="720" src="/vehicle-top.png" width="360" />
          {cameraIds.map((cameraId) => {
            const [x, y] = cameraMounts[cameraId]
            return (
              <span
                aria-hidden="true"
                className="stage__mount"
                data-on={cameraId === activeCamera}
                key={cameraId}
                style={{ left: `${x * 100}%`, top: `${y * 100}%` }}
              />
            )
          })}
          </div>
        </div>
      )}
      {children}
      {extra}
    </main>
  )
}
