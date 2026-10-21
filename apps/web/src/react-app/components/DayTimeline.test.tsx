import { fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { vi } from 'vitest'

import type { ClipCategory, TeslaCamEvent } from '../../teslacam/types'
import { DayTimeline } from './DayTimeline'

function event(
  id: string,
  category: ClipCategory,
  timestamp = '2026-08-24_12-00-00',
): TeslaCamEvent {
  return {
    id,
    category,
    timestamp,
    segments: [{ timestamp, cameras: {} }],
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
        onSelectDate={() => undefined}
        onSelectEvent={onSelectEvent}
      />,
    )

    await user.click(screen.getByRole('button', { name: '12:00–12:00・3件を開く' }))
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
        onSelectDate={() => undefined}
        onSelectEvent={() => undefined}
      />,
    )

    expect(screen.getByLabelText('過去の記録')).toHaveTextContent('セントリー')
    expect(screen.getByLabelText('選択中の記録')).toHaveTextContent('保存済み')
    expect(screen.getByLabelText('新しい記録')).toHaveTextContent('最近の走行')
    expect(screen.getByLabelText('この日の記録数')).toHaveTextContent('3件')
  })

  it('reports an empty local day without rendering marker controls', () => {
    render(
      <DayTimeline
        events={sameSecondEvents}
        filterDateKey="2026-08-25"
        selectedEvent={undefined}
        selectedEventId={null}
        onSelectDate={() => undefined}
        onSelectEvent={() => undefined}
      />,
    )

    expect(screen.getByText('この日のイベントはありません。')).toBeVisible()
    expect(screen.queryByRole('button', { name: /2026-08-25/ })).not.toBeInTheDocument()
  })

  it('moves focus between adjacent marker buttons with horizontal arrow keys', async () => {
    const user = userEvent.setup()
    render(
      <DayTimeline
        events={sameSecondEvents}
        filterDateKey={null}
        selectedEvent={sameSecondEvents[2]}
        selectedEventId="sentry-event"
        onSelectDate={() => undefined}
        onSelectEvent={() => undefined}
      />,
    )

    await user.click(screen.getByRole('button', { name: '12:00–12:00・3件を開く' }))
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

  it('selects the newest event when moving to an earlier recorded day', async () => {
    const previousMorning = event('previous-morning', 'saved', '2026-08-23_08-00-00')
    const previousEvening = event('previous-evening', 'sentry', '2026-08-23_20-00-00')
    const current = event('current', 'recent', '2026-08-24_12-00-00')
    const onSelectDate = vi.fn()
    const user = userEvent.setup()
    render(
      <DayTimeline
        events={[current, previousMorning, previousEvening]}
        filterDateKey={null}
        selectedEvent={current}
        selectedEventId={current.id}
        onSelectDate={onSelectDate}
        onSelectEvent={() => undefined}
      />,
    )

    await user.click(screen.getByRole('button', { name: '前の記録日 2026-08-23' }))

    expect(onSelectDate).toHaveBeenCalledWith({
      dateKey: '2026-08-23',
      eventId: 'previous-evening',
    })
    expect(screen.getByRole('button', { name: '次の記録日' })).toBeDisabled()
  })

  it('expands close events from one cluster without masking their selection targets', async () => {
    const closeEvents = [
      event('sentry-close', 'sentry', '2026-08-24_20-35-00'),
      event('saved-close', 'saved', '2026-08-24_20-40-00'),
      event('recent-close', 'recent', '2026-08-24_20-45-00'),
    ]
    const onSelectEvent = vi.fn()
    const user = userEvent.setup()
    render(
      <DayTimeline
        events={closeEvents}
        filterDateKey={null}
        selectedEvent={closeEvents[0]}
        selectedEventId="sentry-close"
        onSelectDate={() => undefined}
        onSelectEvent={onSelectEvent}
      />,
    )

    await user.click(screen.getByRole('button', { name: '20:35–20:45・3件を開く' }))
    await user.click(screen.getByRole('button', { name: '2026-08-24 20:40:00 保存済み' }))

    expect(onSelectEvent).toHaveBeenCalledWith('saved-close')
    expect(screen.getByRole('button', { name: '20:35–20:45・3件を閉じる' })).toHaveAttribute('aria-expanded', 'true')
  })

  it('moves focus to the first and last record of the day', async () => {
    const events = [
      event('morning', 'saved', '2026-08-24_08-00-00'),
      event('evening', 'recent', '2026-08-24_20-00-00'),
    ]
    render(
      <DayTimeline
        events={events}
        filterDateKey={null}
        selectedEvent={events[0]}
        selectedEventId="morning"
        onSelectDate={() => undefined}
        onSelectEvent={() => undefined}
      />,
    )
    const morning = screen.getByRole('button', { name: /2026-08-24 08:00:00/ })
    const evening = screen.getByRole('button', { name: /2026-08-24 20:00:00/ })

    morning.focus()
    fireEvent.keyDown(morning, { key: 'End' })
    expect(evening).toHaveFocus()
    fireEvent.keyDown(evening, { key: 'Home' })
    expect(morning).toHaveFocus()
  })
})
