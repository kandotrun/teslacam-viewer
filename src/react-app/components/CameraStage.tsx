import type { ReactNode } from 'react'

import type { TheaterViewMode } from './TheaterViewSwitch'

type CameraStageProps = {
  readonly mode: TheaterViewMode
  readonly expanded: boolean
  readonly children: ReactNode
}

export function CameraStage({ mode, expanded, children }: CameraStageProps) {
  return (
    <div className="camera-grid camera-stage" data-expanded={expanded} data-view-mode={mode}>
      {mode === 'spatial' && <img alt="" height="240" src="/vehicle-top.png" width="120" />}
      {children}
    </div>
  )
}
