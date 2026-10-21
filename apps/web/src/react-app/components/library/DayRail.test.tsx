import { fireEvent, render, screen } from '@testing-library/react'
import { afterEach, vi } from 'vitest'

import type { ClipCategory, TeslaCamEvent } from '../../../teslacam/types'
import { mockElementSize } from '../../../test/dom-measurement'
import { buildDayTimelineMarkers } from '../../day-timeline'
import { DayRail } from './DayRail'

function event(id: string, category: ClipCategory, timestamp: string): TeslaCamEvent {
  return { id, category, timestamp, segments: [{ timestamp, cameras: {} }] }
}

const events = [
  event('recent-evening', 'recent', '2026-08-24_20-45-00'),
  event('saved-evening', 'saved', '2026-08-24_20-40-00'),
  event('sentry-evening', 'sentry', '2026-08-24_20-35-00'),
  event('saved-morning', 'saved', '2026-08-24_08-00-00'),
]

function renderRail(width: number, onToggle = vi.fn(), onSelect = vi.fn()) {
  mockElementSize(width)
  render(
    <DayRail
      dayKey="2026-08-24"
      events={events}
      expandedClusterId={null}
      markers={buildDayTimelineMarkers(events, '2026-08-24')}
      selectedEventId="sentry-evening"
      onSelectEvent={onSelect}
      onToggleCluster={onToggle}
    />,
  )
  return { onToggle, onSelect }
}

afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('DayRail', () => {
  it('clusters markers whose hit areas overlap on the measured track', () => {
    const { onToggle, onSelect } = renderRail(332)
    const cluster = screen.getByRole('button', { name: '20:35–20:45・3件を開く' })

    expect(cluster).toHaveAttribute('aria-expanded', 'false')
    expect(cluster).toHaveAttribute('data-current', 'true')
    fireEvent.click(cluster)
    expect(onToggle).toHaveBeenCalledWith(expect.objectContaining({ count: 3 }))
    fireEvent.click(screen.getByRole('button', { name: '2026-08-24 08:00:00 保存済み' }))
    expect(onSelect).toHaveBeenCalledWith('saved-morning')
  })

  it('separates the same markers on a wide track instead of using a fixed width', () => {
    renderRail(40_000)

    expect(screen.queryByRole('button', { name: /件を開く$/u })).not.toBeInTheDocument()
    expect(screen.getByRole('button', { name: '2026-08-24 20:35:00 セントリー' })).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getAllByRole('button')).toHaveLength(4)
  })

  it('moves focus between markers with the arrow keys, Home and End', () => {
    renderRail(332)
    const morning = screen.getByRole('button', { name: '2026-08-24 08:00:00 保存済み' })
    const cluster = screen.getByRole('button', { name: '20:35–20:45・3件を開く' })
    morning.focus()

    fireEvent.keyDown(morning, { key: 'ArrowRight' })
    expect(cluster).toHaveFocus()
    fireEvent.keyDown(cluster, { key: 'Home' })
    expect(morning).toHaveFocus()
    fireEvent.keyDown(morning, { key: 'End' })
    expect(cluster).toHaveFocus()
  })
})
