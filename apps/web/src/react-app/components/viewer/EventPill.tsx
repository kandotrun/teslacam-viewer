import { ArrowLeft, Eye, MapPin, PanelLeftClose, PanelLeftOpen } from 'lucide-react'
import { useEffect, useId, useRef, useState, type ReactNode } from 'react'

import type { TeslaCamEvent } from '../../../teslacam/types'
import { eventTime } from '../../event-filters'
import { useFocusHandoff } from '../../hooks/use-focus-handoff'
import { cameraAvailability, segmentCountLabel } from '../../event-summary'
import {
  cameraCountText,
  coordinatesText,
  detectionSentence,
  durationText,
  locationText,
  missingCamerasText,
  playbackStartText,
  sentryDetection,
  startClock,
} from '../../presentation/event-copy'
import { CategoryChip } from '../common/CategoryChip'
import { NumeralText } from '../common/NumeralText'

type EventPillProps = {
  readonly event: TeslaCamEvent
  /** Phone: the back button to the library. */
  readonly onBack?: (() => void) | undefined
  /** Tablet: shows or hides the library panel. */
  readonly libraryToggle?: { readonly hidden: boolean; readonly onToggle: () => void } | undefined
}

function DetailRow({ term, children }: { readonly term: string; readonly children: ReactNode }) {
  return (
    <div className="details__row">
      <dt>{term}</dt>
      <dd>{children}</dd>
    </div>
  )
}

/** The rows of the details popover, each shown only when its data exists (DESIGN.md §6). */
function EventDetails({ event }: { readonly event: TeslaCamEvent }) {
  const availability = cameraAvailability(event)
  const location = locationText(event.metadata)
  const coordinates = coordinatesText(event.metadata)
  const detection = sentryDetection(event)
  const playbackStart = playbackStartText(event)
  const missing = missingCamerasText(availability)
  const reason = event.metadata?.reason
  return (
    <dl className="details__list">
      {location !== null && <DetailRow term="場所（Teslaの推定値）">{location}</DetailRow>}
      {coordinates !== null && <DetailRow term="推定座標"><span className="num">{coordinates}</span></DetailRow>}
      {detection !== null && <DetailRow term="検知">{detectionSentence(detection)}</DetailRow>}
      {playbackStart !== null && <DetailRow term="再生開始">{playbackStart}</DetailRow>}
      <DetailRow term="カメラ">
        <NumeralText text={cameraCountText(availability)} />
        {missing !== null && <span className="details__warning">{missing}</span>}
      </DetailRow>
      <DetailRow term="長さ"><NumeralText text={durationText(event)} /></DetailRow>
      <DetailRow term="セグメント"><NumeralText text={segmentCountLabel(event)} /></DetailRow>
      <DetailRow term="録画開始"><span className="num">{eventTime(event.timestamp)}</span></DetailRow>
      {reason !== undefined && (
        <div className="details__row details__row--wide">
          <dt className="visually-hidden">記録情報</dt>
          <dd>
            <details aria-label="技術情報" className="details__technical">
              <summary>技術情報</summary>
              <code>{reason}</code>
            </details>
          </dd>
        </div>
      )}
    </dl>
  )
}

/** The event pill: what is playing; activating it opens the details popover. */
export function EventPill({ event, onBack, libraryToggle }: EventPillProps) {
  const [open, setOpen] = useState(false)
  const pill = useRef<HTMLButtonElement>(null)
  const popover = useRef<HTMLDivElement>(null)
  const back = useRef<HTMLButtonElement>(null)
  const popoverId = useId()
  const location = locationText(event.metadata)
  const detection = sentryDetection(event)

  // Phone push: the row that opened the viewer is hidden with the list, so focus continues at 一覧へ.
  useFocusHandoff(back, onBack !== undefined, { onMount: true })

  useEffect(() => {
    if (!open) return undefined
    const closeOnOutside = (pointer: PointerEvent): void => {
      const target = pointer.target
      if (!(target instanceof Node)) return
      if (popover.current?.contains(target) === true || pill.current?.contains(target) === true) return
      setOpen(false)
    }
    document.addEventListener('pointerdown', closeOnOutside)
    return () => document.removeEventListener('pointerdown', closeOnOutside)
  }, [open])

  return (
    <section
      aria-label="再生中のイベント"
      className="event-pill-group"
      onKeyDown={(keyboard) => {
        if (keyboard.key !== 'Escape' || !open) return
        keyboard.stopPropagation()
        setOpen(false)
        pill.current?.focus()
      }}
    >
      {onBack !== undefined && (
        <button className="pill back-button" ref={back} type="button" onClick={onBack}>
          <ArrowLeft aria-hidden="true" size={18} />
          一覧へ
        </button>
      )}
      {libraryToggle !== undefined && (
        <button
          aria-label={libraryToggle.hidden ? 'ライブラリを表示' : 'ライブラリを隠す'}
          aria-pressed={!libraryToggle.hidden}
          className="pill pill--icon"
          type="button"
          onClick={libraryToggle.onToggle}
        >
          {libraryToggle.hidden ? <PanelLeftOpen aria-hidden="true" size={20} /> : <PanelLeftClose aria-hidden="true" size={20} />}
        </button>
      )}
      <button
        aria-controls={open ? popoverId : undefined}
        aria-expanded={open}
        aria-haspopup="dialog"
        className="pill event-pill"
        ref={pill}
        type="button"
        onClick={() => setOpen((current) => !current)}
      >
        <CategoryChip category={event.category} size="large" wording="full" />
        <span className="event-pill__when"><span className="num">{startClock(event.timestamp)}</span><span className="event-pill__suffix"> 開始</span></span>
        {location !== null && (
          <span className="event-pill__where">
            <MapPin aria-hidden="true" size={15} />
            {location}
          </span>
        )}
        {detection !== null && (
          <span className="event-pill__detect">
            <Eye aria-hidden="true" size={15} />
            {detection.reasonWords}<span className="num">{detection.timeLabel}</span>に検知
          </span>
        )}
      </button>
      {open && (
        <div
          aria-label="イベント詳細"
          className="popover details"
          id={popoverId}
          ref={popover}
          role="dialog"
        >
          <EventDetails event={event} />
        </div>
      )}
    </section>
  )
}
