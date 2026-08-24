import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { vi } from 'vitest'

import type { ClipCategory, TeslaCamEvent } from '../../teslacam/types'
import { DayTimeline } from './DayTimeline'

function event(id: string, category: ClipCategory): TeslaCamEvent {
  return {
    id,
    category,
    timestamp: '2026-08-24_12-00-00',
    segments: [{ timestamp: '2026-08-24_12-00-00', cameras: {} }],
  }
}

const sameSecondEvents = [
  event('recent-event', 'recent'),
  event('saved-event', 'saved'),
  event('sentry-event', 'sentry'),
]

describe('DayTimeline', () => {
  it('renders marker buttons in stable order and selects the clicked event', async () => {
    const onSelectEvent = vi.fn()
    const user = userEvent.setup()
    render(
      <DayTimeline
        events={sameSecondEvents}
        filterDateKey={null}
        selectedEvent={sameSecondEvents[2]}
        selectedEventId="sentry-event"
        onSelectEvent={onSelectEvent}
      />,
    )

    const markers = screen.getAllByRole('button', { name: /2026-08-24/ })
    expect(markers.map((marker) => marker.getAttribute('data-day-event-id'))).toEqual([
      'sentry-event',
      'saved-event',
      'recent-event',
    ])
    expect(markers[0]).toHaveAttribute('aria-pressed', 'true')
    expect(markers[1]).toHaveAttribute('aria-pressed', 'false')

    await user.click(screen.getByRole('button', { name: /保存済み/ }))

    expect(onSelectEvent).toHaveBeenCalledWith('saved-event')
  })

  it('summarizes the selected marker with its immediate neighbors', () => {
    render(
      <DayTimeline
        events={sameSecondEvents}
        filterDateKey={null}
        selectedEvent={sameSecondEvents[1]}
        selectedEventId="saved-event"
        onSelectEvent={() => undefined}
      />,
    )

    expect(screen.getByLabelText('前のイベント')).toHaveTextContent('セントリー')
    expect(screen.getByLabelText('選択中のイベント')).toHaveTextContent('保存済み')
    expect(screen.getByLabelText('次のイベント')).toHaveTextContent('最近の走行')
    expect(screen.getByLabelText('この日のイベント数')).toHaveTextContent('3件')
  })

  it('reports an empty local day without rendering marker controls', () => {
    render(
      <DayTimeline
        events={sameSecondEvents}
        filterDateKey="2026-08-25"
        selectedEvent={undefined}
        selectedEventId={null}
        onSelectEvent={() => undefined}
      />,
    )

    expect(screen.getByText('この日のイベントはありません。')).toBeVisible()
    expect(screen.queryByRole('button', { name: /2026-08-25/ })).not.toBeInTheDocument()
  })

  it('moves focus between adjacent marker buttons with horizontal arrow keys', () => {
    render(
      <DayTimeline
        events={sameSecondEvents}
        filterDateKey={null}
        selectedEvent={sameSecondEvents[2]}
        selectedEventId="sentry-event"
        onSelectEvent={() => undefined}
      />,
    )

    const markers = screen.getAllByRole('button', { name: /2026-08-24/ })
    const sentry = markers[0]
    const saved = markers[1]
    const recent = markers[2]
    if (sentry === undefined || saved === undefined || recent === undefined) {
      throw new Error('timeline markers are missing')
    }

    sentry.focus()
    fireEvent.keyDown(sentry, { key: 'ArrowRight' })
    expect(saved).toHaveFocus()
    fireEvent.keyDown(saved, { key: 'ArrowRight' })
    expect(recent).toHaveFocus()
    fireEvent.keyDown(recent, { key: 'ArrowLeft' })
    expect(saved).toHaveFocus()
  })
})
