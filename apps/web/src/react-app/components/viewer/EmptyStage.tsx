import { cameraIds } from '../../../teslacam/types'
import { cameraLabels } from '../../event-summary'
import { StageFrame } from './StageFrame'

/** The car with six empty windows: the stage before a folder is loaded or an event is chosen. */
export function EmptyStage() {
  return (
    <StageFrame activeCamera="front" mode="empty">
      {cameraIds.map((cameraId) => (
        <div aria-hidden="true" className="camera-slot" data-area={cameraId} data-camera={cameraId} key={cameraId} style={{ gridArea: cameraId }}>
          <div className="camera-window camera-window--empty" data-camera={cameraId}>
            <span className="camera-window__label">{cameraLabels[cameraId]}</span>
          </div>
        </div>
      ))}
    </StageFrame>
  )
}
