import { useState } from 'react'

import type { TeslaCamEvent } from '../../teslacam/types'
import { detectionSeconds, sentryPlaybackStartLabel } from '../event-detection'
import { categoryLabel, eventClock, eventTime } from '../event-filters'
import {
  cameraAvailability,
  cameraAvailabilityLabel,
  eventDurationLabel,
  missingCameraLabel,
  segmentCountLabel,
} from '../event-summary'

type EventInspectorProps = {
  readonly event: TeslaCamEvent
}

export function EventInspector({ event }: EventInspectorProps) {
  const [open, setOpen] = useState(() => (
    typeof window !== 'undefined'
    && typeof window.matchMedia === 'function'
    && window.matchMedia('(min-width: 1025px)').matches
  ))
  const availability = cameraAvailability(event)
  const missing = missingCameraLabel(availability)
  const detected = detectionSeconds(event)
  const detectionTimestamp = detected === null ? null : event.metadata?.timestamp ?? null
  const playbackStart = detectionTimestamp === null ? null : sentryPlaybackStartLabel(event)
  const reason = event.metadata?.reason
  const city = event.metadata?.city?.trim() ?? ''
  const street = event.metadata?.street?.trim() ?? ''
  const latitude = event.metadata?.est_lat?.trim() ?? ''
  const longitude = event.metadata?.est_lon?.trim() ?? ''
  const location = [city, street].filter((part) => part.length > 0).join(' ')
  const coordinates = latitude.length > 0 && longitude.length > 0
    ? `${latitude}, ${longitude}`
    : null
  const hasLocation = location.length > 0 || coordinates !== null

  return (
    <details
      aria-label="イベント詳細"
      className="event-inspector"
      data-location={hasLocation ? 'available' : 'unavailable'}
      open={open}
      onToggle={(toggleEvent) => setOpen(toggleEvent.currentTarget.open)}
    >
      <summary>
        <span className="event-inspector__summary-title">
          <span className="event-inspector__summary-default">イベント詳細</span>
          {hasLocation && <span className="event-inspector__summary-location">場所を確認</span>}
        </span>
        <span className="event-inspector__summary-context">
          {categoryLabel(event.category)} · {eventClock(event.timestamp)} · {cameraAvailabilityLabel(availability)}
        </span>
      </summary>
      <dl>
        {location.length > 0 && (
          <div className="event-inspector__location">
            <dt>場所（Teslaの推定値）</dt>
            <dd>{location}</dd>
          </div>
        )}
        {coordinates !== null && (
          <div className="event-inspector__coordinates">
            <dt>推定座標</dt>
            <dd>{coordinates}</dd>
          </div>
        )}
        {detectionTimestamp !== null && (
          <div>
            <dt>検知時刻</dt>
            <dd>{detectionTimestamp}</dd>
          </div>
        )}
        {playbackStart !== null && (
          <div>
            <dt>再生開始</dt>
            <dd>{playbackStart}（検知の15秒前）</dd>
          </div>
        )}
        <div>
          <dt>連結再生時間</dt>
          <dd>{eventDurationLabel(event)}</dd>
        </div>
        <div>
          <dt>セグメント</dt>
          <dd>{segmentCountLabel(event)}</dd>
        </div>
        <div>
          <dt>カメラ</dt>
          <dd>{cameraAvailabilityLabel(availability)}</dd>
        </div>
        {missing !== null && (
          <div>
            <dt>欠損カメラ</dt>
            <dd>{missing}</dd>
          </div>
        )}
        <div>
          <dt>カテゴリ</dt>
          <dd>{categoryLabel(event.category)}</dd>
        </div>
        <div>
          <dt>録画開始</dt>
          <dd>{eventTime(event.timestamp)}</dd>
        </div>
        {reason !== undefined && (
          <div className="event-inspector__technical-row">
            <dt>記録情報</dt>
            <dd>
              <details aria-label="技術情報" className="event-inspector__technical">
                <summary>技術情報</summary>
                <code>{reason}</code>
              </details>
            </dd>
          </div>
        )}
      </dl>
    </details>
  )
}
