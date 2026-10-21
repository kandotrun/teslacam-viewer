import { ChevronLeft, ChevronRight, Search, X } from 'lucide-react'
import { useRef, useState } from 'react'

import type { ScanWarning } from '../../../teslacam/types'
import type { DayTimelineCluster, TimelineDay } from '../../day-timeline'
import type { EventFilters } from '../../event-filters'
import { dayTitle, warningText } from '../../presentation/event-copy'
import type { LibraryView } from '../../presentation/library-view'
import { SegmentedControl, type SegmentedOption } from '../common/SegmentedControl'
import { clusterRange } from '../../presentation/rail-layout'
import { DayRail } from './DayRail'
import { EventRows } from './EventRows'

type LibraryBrowserProps = {
  readonly view: LibraryView
  readonly filters: EventFilters
  readonly selectedEventId: string | null
  readonly warnings: readonly ScanWarning[]
  readonly searchOpen: boolean
  readonly onCategory: (category: EventFilters['category']) => void
  readonly onQuery: (query: string) => void
  readonly onCloseSearch: () => void
  readonly onSelectEvent: (eventId: string) => void
  readonly onSelectDay: (day: TimelineDay) => void
  readonly onPickDate: (dateKey: string | null) => void
}

type OpenCluster = {
  readonly dayKey: string
  readonly id: string
  readonly label: string
  readonly eventIds: readonly string[]
}

function categoryOptions(counts: LibraryView['counts']): readonly SegmentedOption<EventFilters['category']>[] {
  const option = (value: EventFilters['category'], short: string, full: string): SegmentedOption<EventFilters['category']> => ({
    value,
    name: `${full} ${counts[value]}件`,
    label: <>{short}<b className="num">{counts[value]}</b></>,
  })
  return [
    option('all', 'すべて', 'すべて'),
    option('recent', '走行', '最近の走行'),
    option('saved', '保存', '保存済み'),
    option('sentry', 'セントリー', 'セントリー'),
  ]
}

function DayHeader({ view, onSelectDay, onPickDate }: Pick<LibraryBrowserProps, 'view' | 'onSelectDay' | 'onPickDate'>) {
  const input = useRef<HTMLInputElement>(null)
  const title = view.dayKey === null ? null : dayTitle(view.dayKey)

  function openPicker(): void {
    const element = input.current
    if (element === null) return
    if (typeof element.showPicker === 'function') {
      try {
        element.showPicker()
        return
      } catch {
        // Some browsers refuse showPicker outside a direct gesture; focusing reveals the field instead.
      }
    }
    element.focus()
  }

  return (
    <div className="day-nav">
      <button
        aria-label="前の記録日"
        className="icon-button"
        disabled={view.previousDay === undefined}
        type="button"
        onClick={() => {
          if (view.previousDay !== undefined) onSelectDay(view.previousDay)
        }}
      >
        <ChevronLeft aria-hidden="true" size={20} />
      </button>
      <h2 className="day-nav__title">
        <button aria-label={title === null ? '日付を選ぶ' : `日付を選ぶ（${title.date} ${title.weekday}）`} className="day-nav__picker" type="button" onClick={openPicker}>
          {title === null
            ? '日付を選択してください'
            : <><span className="num">{title.month}</span>月<span className="num">{title.day}</span>日<small>{title.weekday}</small></>}
        </button>
        <input
          aria-label="日付"
          className="day-nav__input"
          ref={input}
          tabIndex={-1}
          type="date"
          value={view.dayKey ?? ''}
          onChange={(event) => onPickDate(event.currentTarget.value || null)}
        />
      </h2>
      <button
        aria-label="次の記録日"
        className="icon-button"
        disabled={view.nextDay === undefined}
        type="button"
        onClick={() => {
          if (view.nextDay !== undefined) onSelectDay(view.nextDay)
        }}
      >
        <ChevronRight aria-hidden="true" size={20} />
      </button>
    </div>
  )
}

/** The loaded library: categories, the day on screen with its rail and rows, and scan warnings. */
export function LibraryBrowser(props: LibraryBrowserProps) {
  const { view, filters, selectedEventId, warnings, searchOpen } = props
  const [cluster, setCluster] = useState<OpenCluster | null>(null)
  const openCluster = cluster !== null && cluster.dayKey === view.dayKey ? cluster : null
  const rows = openCluster === null
    ? view.dayEvents
    : view.dayEvents.filter((event) => openCluster.eventIds.includes(event.id))

  function toggleCluster(next: DayTimelineCluster): void {
    if (view.dayKey === null) return
    setCluster(openCluster?.id === next.id
      ? null
      : { dayKey: view.dayKey, id: next.id, label: `${clusterRange(next)}・${next.count}件`, eventIds: next.markers.map((marker) => marker.eventId) })
  }

  return (
    <div className="library__body">
      {(searchOpen || filters.query !== '') && (
        <label className="library__search">
          <Search aria-hidden="true" size={16} />
          <input
            aria-label="検索"
            autoFocus={searchOpen}
            placeholder="日時またはカテゴリ"
            type="search"
            value={filters.query}
            onChange={(event) => props.onQuery(event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === 'Escape' && event.currentTarget.value === '') props.onCloseSearch()
            }}
          />
        </label>
      )}
      <SegmentedControl
        className="segmented library__categories"
        label="映像カテゴリ"
        options={categoryOptions(view.counts)}
        value={filters.category}
        onChange={props.onCategory}
      />
      <section aria-label="記録日" className="day">
        <DayHeader view={view} onPickDate={props.onPickDate} onSelectDay={props.onSelectDay} />
        {view.dayKey !== null && view.markers.length > 0 && (
          <DayRail
            dayKey={view.dayKey}
            events={view.timelineEvents}
            expandedClusterId={openCluster?.id ?? null}
            markers={view.markers}
            selectedEventId={selectedEventId}
            onSelectEvent={props.onSelectEvent}
            onToggleCluster={toggleCluster}
          />
        )}
      </section>
      {openCluster !== null && (
        <div aria-label={openCluster.label} className="cluster-filter" role="group">
          <span className="num">{openCluster.label}</span>
          <button aria-label="絞り込みを解除" className="icon-button" type="button" onClick={() => setCluster(null)}>
            <X aria-hidden="true" size={16} />
          </button>
        </div>
      )}
      {rows.length === 0 ? (
        <p className="rows__empty">
          {view.markers.length === 0 && filters.dateKey !== null ? 'この日のイベントはありません。' : 'この条件に一致するイベントはありません。'}
        </p>
      ) : (
        <EventRows events={rows} selectedEventId={selectedEventId} onSelectEvent={props.onSelectEvent} />
      )}
      {warnings.length > 0 && (
        <footer className="library__foot">
          {warnings.map((warning) => <p key={warning.kind}>{warningText(warning)}</p>)}
        </footer>
      )}
    </div>
  )
}
