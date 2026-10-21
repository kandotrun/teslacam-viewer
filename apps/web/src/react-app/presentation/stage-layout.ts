/**
 * Where each of the six always-mounted camera windows sits on the stage for a given view (DESIGN.md §6
 * Stage). Changing the arrangement only changes grid areas, never the windows themselves.
 */
import { cameraIds, type CameraId } from '../../teslacam/types'

/** The view the user picks in the actions pill. */
export type ViewMode = 'spatial' | 'grid' | 'representative'

export type LayoutBand = 'desktop' | 'tablet' | 'phone'

export type StageMode =
  | 'spatial'
  | 'grid'
  /** One camera fills the stage; the other five form a strip with the mini car. */
  | 'expanded'
  /** `event.mp4` fills the stage; the six cameras form the strip. */
  | 'representative'
  /** Phone: the active camera large with the mini car switcher below. */
  | 'single'

/** Order in which the windows fill with video after loading, front first. */
export const revealOrder = ['front', 'left_pillar', 'right_pillar', 'left_repeater', 'right_repeater', 'back'] as const satisfies readonly CameraId[]

/**
 * The view actually shown: representative when every camera ended in an error and `event.mp4` exists,
 * and never representative without `event.mp4`.
 */
export function effectiveViewMode(requested: ViewMode, hasRepresentative: boolean, everyCameraFailed: boolean): ViewMode {
  if (!hasRepresentative) return requested === 'representative' ? 'spatial' : requested
  return everyCameraFailed ? 'representative' : requested
}

export function stageMode(view: ViewMode, expandedCamera: CameraId | null, band: LayoutBand): StageMode {
  if (view === 'representative') return 'representative'
  if (band === 'phone') return 'single'
  if (expandedCamera !== null) return 'expanded'
  return view
}

/** Grid area of a camera window; `hidden` windows stay mounted but are not displayed. */
export function windowArea(mode: StageMode, cameraId: CameraId, focusCamera: CameraId): string {
  switch (mode) {
    case 'spatial':
    case 'grid':
      return cameraId
    case 'single':
      return cameraId === focusCamera ? 'main' : 'hidden'
    case 'expanded': {
      if (cameraId === focusCamera) return 'main'
      const strip = cameraIds.filter((candidate) => candidate !== focusCamera)
      return `s${strip.indexOf(cameraId) + 1}`
    }
    case 'representative':
      return `s${cameraIds.indexOf(cameraId) + 1}`
  }
}

export function viewModesFor(hasRepresentative: boolean, band: LayoutBand): readonly ViewMode[] {
  const modes: ViewMode[] = band === 'phone' ? ['spatial'] : ['spatial', 'grid']
  if (hasRepresentative) modes.push('representative')
  return modes
}
