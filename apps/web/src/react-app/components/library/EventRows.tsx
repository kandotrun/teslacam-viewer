import { useEffect, useRef, useState } from 'react'

import type { TeslaCamEvent } from '../../../teslacam/types'
import { eventTime } from '../../event-filters'
import { cameraAvailability, eventDurationLabel } from '../../event-summary'
import { focusIsLost } from '../../hooks/use-focus-handoff'
import { useVirtualRows } from '../../hooks/use-virtual-rows'
import {
  cameraCountText,
  detectionFact,
  missingCamerasText,
  sentryDetection,
  startClock,
} from '../../presentation/event-copy'
import { CategoryChip, CategoryIcon } from '../common/CategoryChip'
import { NumeralText } from '../common/NumeralText'

type EventRowsProps = {
  readonly events: readonly TeslaCamEvent[]
  readonly selectedEventId: string | null
  readonly onSelectEvent: (eventId: string) => void
  /**
   * Each new value asks for the selected row (or the first) to take focus once it is mounted, when a
   * screen change left focus nowhere (folder loaded, phone list shown again).
   */
  readonly focusRequest?: number | undefined
}

/** Row height plus gap; rows are fixed-height so the list can be windowed. */
export const EVENT_ROW_STRIDE = 90

/** Loads `thumb.png` while the row is mounted (i.e. near the viewport) and releases it afterwards. */
function EventThumbnail({ event }: { readonly event: TeslaCamEvent }) {
  const [url, setUrl] = useState<string | null>(null)
  const [failed, setFailed] = useState(false)

  useEffect(() => {
    const thumbnailFile = event.thumbnailFile
    if (thumbnailFile === undefined) return undefined
    let mounted = true
    let objectUrl: string | null = null
    void thumbnailFile.getFile().then(
      (file) => {
        if (!mounted) return
        objectUrl = URL.createObjectURL(file)
        setUrl(objectUrl)
      },
      () => {
        if (mounted) setFailed(true)
      },
    )
    return () => {
      mounted = false
      if (objectUrl !== null) URL.revokeObjectURL(objectUrl)
    }
  }, [event.thumbnailFile])

  return (
    <span className="event-row__thumb" data-category={event.category}>
      {url !== null && !failed
        ? <img alt="" className="event-thumbnail" src={url} onError={() => setFailed(true)} />
        : <CategoryIcon category={event.category} size={20} />}
    </span>
  )
}

function EventRow({ event, selected, onSelect }: { readonly event: TeslaCamEvent; readonly selected: boolean; readonly onSelect: () => void }) {
  const availability = cameraAvailability(event)
  const missing = missingCamerasText(availability)
  const detection = sentryDetection(event)
  const describedBy = `${event.id}-facts`
  return (
    <button
      aria-describedby={describedBy}
      aria-label={eventTime(event.timestamp)}
      aria-pressed={selected}
      className="event-row"
      data-selected={selected}
      type="button"
      onClick={onSelect}
    >
      <EventThumbnail event={event} />
      <span className="event-row__body">
        <span className="event-row__head">
          <span className="num event-row__time">{startClock(event.timestamp)}</span>
          <CategoryChip category={event.category} wording="short" />
        </span>
        <span className="event-row__facts" id={describedBy}>
          <span><NumeralText text={eventDurationLabel(event)} /></span>{' '}
          <span><NumeralText text={cameraCountText(availability)} /></span>
          {detection !== null && <>{' '}<span>{detectionFact(detection)}</span></>}
          {missing !== null && <>{' '}<span className="event-row__warning">{missing}</span></>}
        </span>
      </span>
    </button>
  )
}

/** The day's events, newest first, windowed so only rows near the viewport are mounted. */
export function EventRows({ events, selectedEventId, onSelectEvent, focusRequest }: EventRowsProps) {
  const scrollerRef = useRef<HTMLDivElement>(null)
  const rows = useVirtualRows(scrollerRef, events.length, EVENT_ROW_STRIDE)
  const selectedIndex = events.findIndex((event) => event.id === selectedEventId)
  const { scrollToIndex } = rows
  const focusIndex = Math.max(0, selectedIndex)
  const handledFocusRequest = useRef<number | undefined>(undefined)

  // Keyed on the id too: another day's newest event sits at the same index as the previous selection.
  useEffect(() => {
    scrollToIndex(selectedIndex)
  }, [scrollToIndex, selectedEventId, selectedIndex])

  useEffect(() => {
    if (focusRequest !== handledFocusRequest.current) scrollToIndex(focusIndex)
  }, [focusIndex, focusRequest, scrollToIndex])

  // Runs after every render: the row to focus may only mount once the scroll above has been applied.
  useEffect(() => {
    if (focusRequest === undefined || focusRequest === handledFocusRequest.current) return
    const row = scrollerRef.current?.querySelector<HTMLElement>(`[data-index="${focusIndex}"] .event-row`)
    if (row === null || row === undefined) return
    handledFocusRequest.current = focusRequest
    if (focusIsLost()) row.focus()
  })

  return (
    <div aria-label="イベント一覧" className="rows" ref={scrollerRef} role="group">
      <div className="rows__spacer" style={{ height: `${rows.totalHeight}px` }}>
        {events.slice(rows.start, rows.end).map((event, offset) => (
          <div className="rows__item" data-index={rows.start + offset} key={event.id} style={{ transform: `translateY(${(rows.start + offset) * EVENT_ROW_STRIDE}px)` }}>
            <EventRow event={event} selected={event.id === selectedEventId} onSelect={() => onSelectEvent(event.id)} />
          </div>
        ))}
      </div>
    </div>
  )
}
