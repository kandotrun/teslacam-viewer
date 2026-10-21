import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import type { TeslaCamEvent } from '../../teslacam/types'
import { EventInspector } from './EventInspector'

const eventWithLocationMetadata: TeslaCamEvent = {
  id: 'sentry-location',
  category: 'sentry',
  timestamp: '2026-08-20_20-45-00',
  metadata: {
    timestamp: '2026-08-20T20:45:15',
    reason: 'sentry_aware_object_detection',
    city: 'Sample city',
    street: 'Sample street',
    est_lat: '35.0000',
    est_lon: '139.0000',
  },
  segments: [
    {
      timestamp: '2026-08-20_20-45-00',
      cameras: {},
    },
  ],
}

describe('EventInspector', () => {
  it('prioritizes SSD location while keeping technical detection data collapsed', () => {
    render(<EventInspector event={eventWithLocationMetadata} />)

    const details = screen.getByLabelText('イベント詳細')
    expect(screen.getByText('セントリー')).toBeInTheDocument()
    expect(screen.getByText('録画開始')).toBeInTheDocument()
    expect(screen.getByText('検知時刻')).toBeInTheDocument()
    expect(screen.getByText('再生開始')).toBeInTheDocument()
    expect(screen.getByText('連結再生時間')).toBeInTheDocument()
    expect(screen.getByText('場所（Teslaの推定値）')).toBeInTheDocument()
    expect(screen.getByText('Sample city Sample street')).toBeInTheDocument()
    expect(screen.getByText('推定座標')).toBeInTheDocument()
    expect(screen.getByText('35.0000, 139.0000')).toBeInTheDocument()
    expect(screen.getByRole('group', { name: '技術情報' })).not.toHaveAttribute('open')
    expect(screen.getByText('sentry_aware_object_detection')).not.toBeVisible()
    expect(details).toHaveAttribute('data-location', 'available')
  })

  it('reveals the detection reason from technical details', async () => {
    const user = userEvent.setup()
    render(<EventInspector event={eventWithLocationMetadata} />)
    const inspector = screen.getByLabelText('イベント詳細')
    inspector.setAttribute('open', '')

    await user.click(screen.getByText('技術情報'))

    expect(screen.getByRole('group', { name: '技術情報' })).toHaveAttribute('open')
    expect(screen.getByText('sentry_aware_object_detection')).toBeVisible()
  })

  it('shows an available place without presenting an incomplete coordinate pair', () => {
    render(<EventInspector event={{
      ...eventWithLocationMetadata,
      id: 'sentry-partial-location',
      metadata: {
        timestamp: '2026-08-20T20:45:15',
        street: 'Sample street',
        est_lat: '35.0000',
      },
    }} />)

    expect(screen.getByText('場所（Teslaの推定値）')).toBeInTheDocument()
    expect(screen.getByText('Sample street')).toBeInTheDocument()
    expect(screen.queryByText('推定座標')).not.toBeInTheDocument()
    expect(screen.getByLabelText('イベント詳細')).toHaveAttribute('data-location', 'available')
  })

  it('exposes a native disclosure for compact investigation layouts', async () => {
    const user = userEvent.setup()
    render(<EventInspector event={eventWithLocationMetadata} />)

    const details = screen.getByLabelText('イベント詳細')
    expect(details).not.toHaveAttribute('open')
    const summaryContext = details.querySelector<HTMLElement>('.event-inspector__summary-context')
    if (summaryContext === null) throw new Error('event inspector summary context is missing')
    expect(summaryContext).toHaveTextContent('セントリー')
    expect(summaryContext).toHaveTextContent('20:45:00')
    expect(summaryContext).toHaveTextContent('0/6 カメラ')
    await user.click(screen.getByText('イベント詳細'))
    expect(details).toHaveAttribute('open')
  })
})
