import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, vi } from 'vitest'

import { cameraIds } from '../../teslacam/types'
import { Theater } from './Theater.test-harness'
import { eventWithCameras, firstCameraVideo, labels, sentryEventWithCameras, setupTheaterMedia } from './Theater.test-fixtures'

beforeEach(setupTheaterMedia)
afterEach(() => vi.restoreAllMocks())

describe('Theater layout', () => {
  it('autoplays sentry playback at its pre-roll location once ready', async () => {
    render(<Theater event={sentryEventWithCameras('sentry-preroll')} onPrevious={() => undefined} onNext={() => undefined} hasPrevious hasNext />)
    expect(screen.getByLabelText('再生位置')).toHaveValue('65')
    const firstVideo = firstCameraVideo(screen.getAllByTestId('camera-video'))
    fireEvent.canPlay(firstVideo)
    await waitFor(() => expect(screen.getByRole('button', { name: '一時停止' })).toBeVisible())
    expect(firstVideo.currentTime).toBe(5)
  })

  it('keeps the selected evidence context and its actions in one surface', () => {
    render(<Theater event={eventWithCameras()} hasNext hasPrevious onNext={() => undefined} onPrevious={() => undefined} />)
    const context = screen.getByLabelText('再生中のイベント')
    expect(context).toHaveTextContent('2026-08-21 12:00:00')
    expect(context).toHaveTextContent('保存済み')
    expect(context).toHaveTextContent('6/6 カメラ')
    expect(context).toHaveTextContent('1分')
    expect(context).toContainElement(screen.getByRole('button', { name: '合成動画を書き出す' }))
    expect(context).toContainElement(screen.getByRole('button', { name: '車両配置' }))
    const stage = document.querySelector('[data-view-mode="spatial"]')
    expect(stage?.parentElement).toContainElement(screen.getByLabelText('イベント詳細'))
  })

  it('switches the same six camera nodes between spatial and grid modes without reacquiring media', async () => {
    const user = userEvent.setup()
    render(<Theater event={eventWithCameras()} onPrevious={() => undefined} onNext={() => undefined} hasPrevious hasNext />)
    await waitFor(() => expect(URL.createObjectURL).toHaveBeenCalledTimes(6))
    const videos = screen.getAllByTestId('camera-video')
    expect(screen.getByRole('button', { name: '車両配置' })).toHaveAttribute('aria-pressed', 'true')
    await user.click(screen.getByRole('button', { name: '6分割' }))
    expect(screen.getAllByTestId('camera-video')).toEqual(videos)
    expect(URL.createObjectURL).toHaveBeenCalledTimes(6)
    expect(URL.revokeObjectURL).not.toHaveBeenCalled()
    await user.click(screen.getByRole('button', { name: '車両配置' }))
    expect(URL.createObjectURL).toHaveBeenCalledTimes(6)
    await user.click(screen.getByRole('button', { name: '6分割' }))
    await user.click(screen.getByRole('button', { name: '左ピラーカメラを拡大' }))
    expect(screen.getByRole('button', { name: '車両配置' })).toBeDisabled()
    expect(screen.getAllByTestId('camera-video')).toHaveLength(6)
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(screen.getByRole('button', { name: '6分割' })).toHaveAttribute('aria-pressed', 'true')
  })

  it('preserves the selected view mode while an event-keyed session reacquires media', async () => {
    const user = userEvent.setup()
    const view = render(<Theater event={eventWithCameras(cameraIds, 'first')} onPrevious={() => undefined} onNext={() => undefined} hasPrevious hasNext />)
    await waitFor(() => expect(URL.createObjectURL).toHaveBeenCalledTimes(6))
    await user.click(screen.getByRole('button', { name: '6分割' }))
    view.rerender(<Theater event={eventWithCameras(cameraIds, 'second')} onPrevious={() => undefined} onNext={() => undefined} hasPrevious hasNext />)
    expect(screen.getByRole('button', { name: '6分割' })).toHaveAttribute('aria-pressed', 'true')
    await waitFor(() => expect(URL.createObjectURL).toHaveBeenCalledTimes(12))
    expect(URL.revokeObjectURL).toHaveBeenCalledTimes(6)
  })

  it('offers a labeled six-camera switcher while keeping every video mounted', async () => {
    const user = userEvent.setup()
    render(<Theater event={eventWithCameras()} onPrevious={() => undefined} onNext={() => undefined} hasPrevious hasNext />)
    const switcher = screen.getByRole('group', { name: '表示するカメラ' })
    const rightPillar = screen.getByRole('button', { name: '右ピラーを表示' })
    expect(switcher).toBeVisible()
    expect(screen.getByRole('button', { name: 'フロントを表示' })).toHaveAttribute('aria-pressed', 'true')
    await user.click(rightPillar)
    expect(rightPillar).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getAllByTestId('camera-video')).toHaveLength(6)
  })

  it('renders six cameras in canonical Japanese order and keeps every tile mounted while expanded', async () => {
    const user = userEvent.setup()
    render(<Theater event={eventWithCameras()} onPrevious={() => undefined} onNext={() => undefined} hasPrevious hasNext />)
    const tiles = screen.getAllByRole('button', { name: /カメラを拡大/ })
    expect(tiles.map((tile) => tile.getAttribute('aria-label'))).toEqual(labels.map((label) => `${label}カメラを拡大`))
    expect(tiles.map((tile) => tile.getAttribute('data-camera'))).toEqual(cameraIds)
    const leftPillar = tiles.at(2)
    if (leftPillar === undefined) throw new Error('left pillar tile is missing')
    await user.click(leftPillar)
    expect(screen.getAllByTestId('camera-video')).toHaveLength(6)
    expect(leftPillar).toHaveAttribute('aria-pressed', 'true')
    await user.click(leftPillar)
    expect(leftPillar).toHaveAttribute('aria-pressed', 'false')
  })
})
