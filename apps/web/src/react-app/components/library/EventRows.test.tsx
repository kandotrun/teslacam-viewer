import { act, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, vi } from 'vitest'

import type { CameraId, ClipCategory, TeslaCamEvent } from '../../../teslacam/types'
import { fileRef } from '../../../test/viewer-fixtures'
import { EventRows } from './EventRows'

function event(id: string, category: ClipCategory, timestamp: string, cameras: readonly CameraId[] = ['front', 'back', 'left_pillar', 'left_repeater', 'right_pillar', 'right_repeater']): TeslaCamEvent {
  return {
    id,
    category,
    timestamp,
    segments: [{ timestamp, cameras: Object.fromEntries(cameras.map((camera) => [camera, fileRef(`${camera}.mp4`)])) }],
  }
}

afterEach(() => vi.restoreAllMocks())

describe('EventRows', () => {
  it('shows the start time, category and facts of each event', async () => {
    const user = userEvent.setup()
    const onSelect = vi.fn()
    const sentry: TeslaCamEvent = {
      ...event('sentry-1', 'sentry', '2026-08-20_20-35-00', ['front', 'back', 'left_pillar', 'left_repeater', 'right_pillar']),
      metadata: { timestamp: '2026-08-20T20:35:30' },
    }
    render(<EventRows events={[event('recent-1', 'recent', '2026-08-20_20-45-00'), sentry]} selectedEventId="sentry-1" onSelectEvent={onSelect} />)

    const recentRow = screen.getByRole('button', { name: '2026-08-20 20:45:00' })
    const sentryRow = screen.getByRole('button', { name: '2026-08-20 20:35:00' })
    expect(recentRow).toHaveTextContent('20:45走行1分 6カメラ')
    expect(recentRow).toHaveAttribute('aria-pressed', 'false')
    expect(sentryRow).toHaveTextContent('20:35セントリー1分 5カメラ 20:35:30に検知 右リピーターの記録がありません')
    expect(sentryRow).toHaveAttribute('aria-pressed', 'true')
    expect(sentryRow).toHaveAccessibleDescription('1分 5カメラ 20:35:30に検知 右リピーターの記録がありません')
    expect(within(sentryRow).queryByText(/台のカメラがありません/u)).not.toBeInTheDocument()
    await user.click(recentRow)
    expect(onSelect).toHaveBeenCalledWith('recent-1')
  })

  it('never shows a detection for saved events', () => {
    const saved: TeslaCamEvent = { ...event('saved-1', 'saved', '2026-08-20_20-40-00'), metadata: { timestamp: '2026-08-20T20:40:30' } }
    render(<EventRows events={[saved]} selectedEventId={null} onSelectEvent={() => undefined} />)

    expect(screen.getByRole('button', { name: '2026-08-20 20:40:00' })).not.toHaveTextContent('検知')
  })

  it('mounts only a window of rows for a long day', () => {
    const events = Array.from({ length: 500 }, (_, index) => event(`recent-${index}`, 'recent', `2026-08-20_${String(23 - Math.floor(index / 60)).padStart(2, '0')}-${String(59 - (index % 60)).padStart(2, '0')}-00`))
    render(<EventRows events={events} selectedEventId={null} onSelectEvent={() => undefined} />)

    expect(screen.getAllByRole('button').length).toBeLessThan(20)
    expect(document.querySelector('.rows__spacer')).toHaveStyle({ height: `${500 * 90}px` })
  })

  it('loads thumb.png while the row is mounted and releases it afterwards', async () => {
    const createObjectURL = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:thumbnail')
    const revokeObjectURL = vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => undefined)
    const withThumbnail: TeslaCamEvent = { ...event('saved-1', 'saved', '2026-08-20_20-40-00'), thumbnailFile: fileRef('thumb.png') }
    const view = render(<EventRows events={[withThumbnail]} selectedEventId={null} onSelectEvent={() => undefined} />)

    await waitFor(() => expect(document.querySelector('img.event-thumbnail')).toHaveAttribute('src', 'blob:thumbnail'))
    expect(createObjectURL).toHaveBeenCalledTimes(1)
    view.unmount()
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:thumbnail')
  })

  it('shows a quiet category tile when there is no thumbnail or it cannot be read', async () => {
    let reject: (reason: Error) => void = () => undefined
    const pending = new Promise<File>((_, fail) => {
      reject = fail
    })
    const withBrokenThumbnail: TeslaCamEvent = {
      ...event('saved-1', 'saved', '2026-08-20_20-40-00'),
      thumbnailFile: fileRef('thumb.png', async () => pending),
    }
    render(<EventRows events={[withBrokenThumbnail, event('recent-1', 'recent', '2026-08-20_20-30-00')]} selectedEventId={null} onSelectEvent={() => undefined} />)
    await act(async () => {
      reject(new Error('unreadable'))
      await pending.catch(() => undefined)
    })

    expect(document.querySelector('img.event-thumbnail')).not.toBeInTheDocument()
    expect(document.querySelectorAll('.event-row__thumb svg')).toHaveLength(2)
  })
})
