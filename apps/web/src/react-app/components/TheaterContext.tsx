import type { ReactNode } from 'react'

import type { TeslaCamEvent } from '../../teslacam/types'
import { categoryLabel, eventTime } from '../event-filters'
import {
  cameraAvailability,
  cameraAvailabilityLabel,
  eventDurationLabel,
  missingCameraLabel,
  segmentCountLabel,
} from '../event-summary'

type TheaterContextProps = {
  readonly children: ReactNode
  readonly event: TeslaCamEvent
}

export function TheaterContext({ children, event }: TheaterContextProps) {
  const availability = cameraAvailability(event)
  const missing = missingCameraLabel(availability)

  return (
    <header className="theater-context" aria-label="再生中のイベント">
      <div className="theater-context__event">
        <h2>{eventTime(event.timestamp)}</h2>
        <span className={`event-category event-category--${event.category}`}>
          {categoryLabel(event.category)}
        </span>
      </div>
      <div className="theater-context__facts">
        <span>{eventDurationLabel(event)}</span>
        <span>{segmentCountLabel(event)}</span>
        <span>{cameraAvailabilityLabel(availability)}</span>
        {missing !== null && <span className="theater-context__warning">{missing}</span>}
      </div>
      <div className="theater-context__actions">{children}</div>
    </header>
  )
}
