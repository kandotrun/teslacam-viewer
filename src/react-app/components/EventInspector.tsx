import type { TeslaCamEvent } from '../../teslacam/types'
import { detectionSeconds, sentryPlaybackStartLabel } from '../event-detection'
import { categoryLabel, eventTime } from '../event-filters'
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
  const availability = cameraAvailability(event)
  const missing = missingCameraLabel(availability)
  const detected = detectionSeconds(event)
  const detectionTimestamp = detected === null ? null : event.metadata?.timestamp ?? null
  const playbackStart = detectionTimestamp === null ? null : sentryPlaybackStartLabel(event)
  const reason = event.metadata?.reason

  return (
    <section className="event-inspector" aria-label="イベント詳細">
      <dl>
        <div>
          <dt>カテゴリ</dt>
          <dd>{categoryLabel(event.category)}</dd>
        </div>
        <div>
          <dt>録画開始</dt>
          <dd>{eventTime(event.timestamp)}</dd>
        </div>
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
        {reason !== undefined && (
          <div>
            <dt>reason</dt>
            <dd>{reason}</dd>
          </div>
        )}
      </dl>
    </section>
  )
}
