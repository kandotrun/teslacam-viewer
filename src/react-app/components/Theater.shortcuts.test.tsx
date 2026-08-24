import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, vi } from 'vitest'

import type { TeslaCamEvent } from '../../teslacam/types'
import { Theater } from './Theater.test-harness'
import { eventWithCameras, setupTheaterMedia } from './Theater.test-fixtures'

beforeEach(setupTheaterMedia)
afterEach(() => vi.restoreAllMocks())

describe('Theater playback controls', () => {
  it('supports digit expansion, Escape grid, Space playback, and arrow skipping outside editable controls', async () => {
    render(<Theater event={eventWithCameras()} onPrevious={() => undefined} onNext={() => undefined} hasPrevious hasNext />)
    fireEvent.keyDown(window, { key: '6' })
    expect(screen.getByRole('button', { name: '右リピーターカメラをグリッドへ戻す' })).toHaveAttribute('aria-pressed', 'true')
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(screen.getByRole('button', { name: '右リピーターカメラを拡大' })).toHaveAttribute('aria-pressed', 'false')
    for (const videoElement of screen.getAllByTestId('camera-video')) fireEvent.canPlay(videoElement)
    fireEvent.keyDown(window, { key: ' ' })
    await waitFor(() => expect(screen.getByRole('button', { name: '一時停止' })).toBeVisible())
    fireEvent.keyDown(window, { key: 'ArrowRight' })
    expect(screen.getByLabelText('再生位置')).toHaveValue('10')
  })

  it('returns a clicked and focused expanded camera to the grid with Escape while ignoring other shortcuts', async () => {
    const user = userEvent.setup()
    render(<Theater event={eventWithCameras()} onPrevious={() => undefined} onNext={() => undefined} hasPrevious hasNext />)
    const front = screen.getByRole('button', { name: 'フロントカメラを拡大' })
    await user.click(front)
    expect(front).toHaveFocus()
    fireEvent.keyDown(front, { key: 'ArrowRight' })
    fireEvent.keyDown(front, { key: '6' })
    expect(screen.getByLabelText('再生位置')).toHaveValue('0')
    expect(screen.queryByRole('button', { name: '右リピーターカメラをグリッドへ戻す' })).not.toBeInTheDocument()
    fireEvent.keyDown(front, { key: 'Escape' })
    expect(front).toHaveAttribute('aria-pressed', 'false')
  })

  it('shows transport, speed choices, navigation edges, and an in-range detection marker', async () => {
    const previous = vi.fn()
    const next = vi.fn()
    const user = userEvent.setup()
    render(<Theater event={eventWithCameras()} onPrevious={previous} onNext={next} hasPrevious={false} hasNext />)
    expect(screen.getByRole('button', { name: '前のイベント' })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: '次のイベント' }))
    expect(next).toHaveBeenCalledTimes(1)
    expect(screen.getByLabelText('再生位置')).toHaveAttribute('max', '60')
    const playbackRate = screen.getByRole('combobox', { name: '再生速度' })
    expect(playbackRate).toHaveValue('1')
    await user.selectOptions(playbackRate, '1.5')
    expect(playbackRate).toHaveValue('1.5')
    expect(screen.getByLabelText('検知位置')).toHaveAttribute('style', expect.stringContaining('50%'))
  })

  it('positions a marker by the matching segment rather than event start time', () => {
    const event: TeslaCamEvent = {
      ...eventWithCameras(),
      category: 'sentry',
      metadata: { timestamp: '2026-08-21T12:01:20' },
      segments: [
        { timestamp: '2026-08-21_12-00-00', cameras: {} },
        { timestamp: '2026-08-21_12-01-00', cameras: {} },
      ],
    }
    render(<Theater event={event} onPrevious={() => undefined} onNext={() => undefined} hasPrevious hasNext />)
    expect(screen.getByLabelText('検知位置')).toHaveAttribute('style', expect.stringContaining('66.666'))
  })

  it('omits invalid or out-of-range detection metadata and ignores shortcuts from controls', () => {
    const outsideEvent: TeslaCamEvent = { ...eventWithCameras(), metadata: { timestamp: '2026-08-21T12:02:00' } }
    render(<Theater event={outsideEvent} onPrevious={() => undefined} onNext={() => undefined} hasPrevious hasNext />)
    const playButton = screen.getByRole('button', { name: '再生' })
    fireEvent.keyDown(playButton, { key: 'ArrowRight' })
    expect(screen.queryByLabelText('検知位置')).not.toBeInTheDocument()
    expect(screen.getByLabelText('再生位置')).toHaveValue('0')
  })
})
