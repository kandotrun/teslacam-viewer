import { render, screen } from '@testing-library/react'

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
  it('renders only factual event fields and omits location metadata', () => {
    render(<EventInspector event={eventWithLocationMetadata} />)

    expect(screen.getByText('セントリー')).toBeVisible()
    expect(screen.getByText('録画開始')).toBeVisible()
    expect(screen.getByText('検知時刻')).toBeVisible()
    expect(screen.getByText('再生開始')).toBeVisible()
    expect(screen.getByText('連結再生時間')).toBeVisible()
    expect(screen.getByText('sentry_aware_object_detection')).toBeVisible()
    expect(screen.queryByText('Sample city')).not.toBeInTheDocument()
    expect(screen.queryByText('Sample street')).not.toBeInTheDocument()
    expect(screen.queryByText('35.0000')).not.toBeInTheDocument()
  })
})
