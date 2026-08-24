import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import type { TeslaCamEvent } from '../../teslacam/types'
import { EventList } from './EventList'
import { EventListHarness } from './EventList.test-harness'
import { events, fileRef, firstEvent } from './EventList.test-fixtures'

describe('EventList event summaries', () => {
  it('exposes an event-list disclosure control', async () => {
    const user = userEvent.setup()
    render(<EventListHarness />)
    const disclosure = screen.getByRole('button', { name: 'イベント一覧を閉じる' })
    await user.click(disclosure)
    expect(disclosure).toHaveAccessibleName('イベント一覧を開く')
    expect(disclosure).toHaveAttribute('aria-expanded', 'false')
  })

  it('lists matching events newest first with their missing camera count', () => {
    render(<EventList events={events} selectedEventId={null} onSelectEvent={() => undefined} />)
    const eventButtons = screen.getAllByRole('button', { name: /2026-08/ })
    expect(eventButtons).toHaveLength(2)
    expect(eventButtons[0]).toHaveTextContent('2026-08-20')
    expect(eventButtons[0]).toHaveTextContent('20:45:00')
    expect(eventButtons[0]).toHaveTextContent('1/6 カメラ')
    expect(eventButtons[0]).toHaveTextContent('1分')
    expect(eventButtons[0]).toHaveTextContent('1セグメント')
    expect(eventButtons[0]).not.toHaveTextContent('検知の15秒前から開始')
  })

  it('splits the selected Sentry timestamp and preserves its category as an atomic label', () => {
    render(<EventList events={events} selectedEventId="sentry-newer" onSelectEvent={() => undefined} />)
    const selectedEvent = screen.getByRole('button', { name: /2026-08-20 20:45:00/ })
    const category = screen.getByText('セントリー')
    expect(selectedEvent).toHaveTextContent('2026-08-20')
    expect(selectedEvent).toHaveTextContent('20:45:00')
    expect(category).toHaveClass('event-category')
    expect(category).toHaveAttribute('data-atomic', 'true')
    expect(selectedEvent).toHaveTextContent('検知の15秒前から開始')
  })

  it('states camera availability positively and names missing directions only for partial events', () => {
    render(<EventList events={events} selectedEventId={null} onSelectEvent={() => undefined} />)
    const partialEvent = screen.getByRole('button', { name: /2026-08-20 20:45:00/ })
    expect(partialEvent).toHaveTextContent('1/6 カメラ')
    expect(partialEvent).toHaveTextContent('リア、左ピラー、左リピーター、右ピラー、右リピーターなし')
    expect(partialEvent).not.toHaveTextContent('台のカメラがありません')
  })

  it('renders the matching subset supplied by the workspace', () => {
    render(<EventList events={[firstEvent()]} selectedEventId={null} onSelectEvent={() => undefined} />)
    expect(screen.getByText('2026-08-20')).toBeVisible()
    expect(screen.getByText('20:45:00')).toBeVisible()
    expect(screen.queryByText('2026-08-19')).not.toBeInTheDocument()
  })

  it('selects an event through the rendered event control', async () => {
    const user = userEvent.setup()
    render(<EventListHarness />)
    await user.click(screen.getByRole('button', { name: /2026-08-20 20:45:00/ }))
    expect(screen.getByRole('status')).toHaveTextContent('sentry-newer')
  })

  it('shows an explicit empty result for an empty category', () => {
    render(<EventList events={[]} selectedEventId={null} onSelectEvent={() => undefined} />)
    expect(screen.getByText('この条件に一致するイベントはありません。')).toBeVisible()
  })

  it('reports cameras missing from a later partial segment', () => {
    const fullSegment = {
      timestamp: '2026-08-20_20-45-00',
      cameras: {
        front: fileRef('front.mp4'),
        back: fileRef('back.mp4'),
        left_pillar: fileRef('left_pillar.mp4'),
        left_repeater: fileRef('left_repeater.mp4'),
        right_pillar: fileRef('right_pillar.mp4'),
        right_repeater: fileRef('right_repeater.mp4'),
      },
    }
    const partialSegment = { timestamp: '2026-08-20_20-46-00', cameras: { front: fileRef('later-front.mp4') } }
    const multiSegmentEvent: TeslaCamEvent = {
      id: 'recent-multi-segment',
      category: 'recent',
      timestamp: '2026-08-20_20-45-00',
      segments: [fullSegment, partialSegment],
    }
    render(<EventList events={[multiSegmentEvent]} selectedEventId={null} onSelectEvent={() => undefined} />)
    expect(screen.getByRole('button', { name: /2026-08-20 20:45:00/ })).toHaveTextContent('1/6 カメラ')
  })
})
