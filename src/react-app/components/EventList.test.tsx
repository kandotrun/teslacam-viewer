import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'

import type { TeslaCamEvent } from '../../teslacam/types'
import { EventList } from './EventList'

const events: readonly TeslaCamEvent[] = [
  {
    id: 'recent-older',
    category: 'recent',
    timestamp: '2026-08-19_08-30-00',
    segments: [
      {
        timestamp: '2026-08-19_08-30-00',
        cameras: { front: { name: 'front.mp4', relativePath: 'RecentClips/2026-08-19_08-30-00-front.mp4', size: 12, type: 'video/mp4', getFile: async () => new File(['clip'], 'front.mp4') } },
      },
    ],
  },
  {
    id: 'sentry-newer',
    category: 'sentry',
    timestamp: '2026-08-20_20-45-00',
    segments: [
      {
        timestamp: '2026-08-20_20-45-00',
        cameras: { front: { name: 'front.mp4', relativePath: 'SentryClips/event/2026-08-20_20-45-00-front.mp4', size: 12, type: 'video/mp4', getFile: async () => new File(['clip'], 'front.mp4') } },
      },
    ],
  },
]

function EventListHarness() {
  const [selectedEventId, setSelectedEventId] = useState<string | null>(null)

  return (
    <>
      <EventList
        events={events}
        filters={{ category: 'all', dateKey: null, query: '' }}
        selectedEventId={selectedEventId}
        onSelectEvent={setSelectedEventId}
      />
      <output>{selectedEventId ?? '未選択'}</output>
    </>
  )
}

describe('EventList', () => {
  it('lists matching events newest first with their missing camera count', () => {
    render(
      <EventList
        events={events}
        filters={{ category: 'all', dateKey: null, query: '' }}
        selectedEventId={null}
        onSelectEvent={() => undefined}
      />,
    )

    const eventButtons = screen.getAllByRole('button', { name: /2026-08/ })

    expect(eventButtons).toHaveLength(2)
    expect(eventButtons[0]).toHaveTextContent('2026-08-20 20:45:00')
    expect(eventButtons[0]).toHaveTextContent('5台のカメラがありません')
  })

  it('filters by category, date, and text without exposing location metadata', () => {
    render(
      <EventList
        events={events}
        filters={{ category: 'sentry', dateKey: '2026-08-20', query: '20:45' }}
        selectedEventId={null}
        onSelectEvent={() => undefined}
      />,
    )

    expect(screen.getByText('2026-08-20 20:45:00')).toBeVisible()
    expect(screen.queryByText('2026-08-19 08:30:00')).not.toBeInTheDocument()
  })

  it('selects an event through the rendered event control', async () => {
    const user = userEvent.setup()
    render(<EventListHarness />)

    await user.click(screen.getByRole('button', { name: /2026-08-20 20:45:00/ }))

    expect(screen.getByRole('status')).toHaveTextContent('sentry-newer')
  })

  it('shows an explicit empty result for an empty category', () => {
    render(
      <EventList
        events={events}
        filters={{ category: 'saved', dateKey: null, query: '' }}
        selectedEventId={null}
        onSelectEvent={() => undefined}
      />,
    )

    expect(screen.getByText('この条件に一致するイベントはありません。')).toBeVisible()
  })
})
