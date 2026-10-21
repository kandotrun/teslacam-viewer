import { cameraIds, type CameraId } from '../../../teslacam/types'
import { cameraLabels } from '../../event-summary'

type CameraSwitcherProps = {
  readonly activeCamera: CameraId
  readonly onSelect: (cameraId: CameraId) => void
  /** `labeled` (phone) shows each camera's name; `compact` (expanded strip) shows the mount dots only. */
  readonly variant: 'labeled' | 'compact'
}

/** The mini car: the car glyph with one button per camera at its place around the car. */
export function CameraSwitcher({ activeCamera, onSelect, variant }: CameraSwitcherProps) {
  return (
    <div aria-label="表示するカメラ" className="camera-switcher" data-variant={variant} role="group">
      <img alt="" className="camera-switcher__car" draggable={false} height="120" src="/vehicle-top.png" width="60" />
      {cameraIds.map((cameraId) => (
        <button
          aria-label={`${cameraLabels[cameraId]}を表示`}
          aria-pressed={cameraId === activeCamera}
          className="camera-switcher__camera"
          data-camera={cameraId}
          key={cameraId}
          type="button"
          onClick={() => onSelect(cameraId)}
        >
          <span aria-hidden="true" className="camera-switcher__dot" />
          <span className="camera-switcher__label">{cameraLabels[cameraId]}</span>
        </button>
      ))}
    </div>
  )
}
