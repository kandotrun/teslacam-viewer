import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, vi } from 'vitest'

import { WorkspaceSelectionHarness } from './App.test-support'

function firstCameraVideo(): HTMLVideoElement {
  const video = screen.getAllByTestId('camera-video').find((element): element is HTMLVideoElement => element instanceof HTMLVideoElement)
  if (video === undefined) throw new Error('first camera video is missing')
  return video
}

afterEach(() => vi.restoreAllMocks())

describe('workspace event selection', () => {
  it('autoplays fresh playback for list and previous-next sentry selections without rewinding a reselected event', async () => {
    const user = userEvent.setup()
    vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:camera')
    vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => undefined)
    vi.spyOn(HTMLMediaElement.prototype, 'load').mockImplementation(() => undefined)
    vi.spyOn(HTMLMediaElement.prototype, 'pause').mockImplementation(() => undefined)
    vi.spyOn(HTMLMediaElement.prototype, 'play').mockImplementation(async () => undefined)
    const view = render(<WorkspaceSelectionHarness />)

    const firstVideo = firstCameraVideo()
    fireEvent.canPlay(firstVideo)
    expect(screen.getByLabelText('再生位置')).toHaveValue('65')
    await waitFor(() => expect(screen.getByRole('button', { name: '一時停止' })).toBeVisible())
    expect(firstVideo.currentTime).toBe(5)

    firstVideo.currentTime = 9
    const timelineMarker = document.querySelector<HTMLButtonElement>('[data-day-event-id="sentry-preroll"]')
    if (timelineMarker === null) throw new Error('timeline marker is missing')
    await user.click(timelineMarker)
    expect(firstVideo.currentTime).toBe(9)
    const listEvent = screen.getAllByRole('button', { name: /2026-08-21 12:00:00/ }).find((button) => button.classList.contains('event-card'))
    if (listEvent === undefined) throw new Error('event list selection is missing')
    await user.click(listEvent)
    expect(firstVideo.currentTime).toBe(9)
    const nextTimelineMarker = document.querySelector<HTMLButtonElement>('[data-day-event-id="sentry-start"]')
    if (nextTimelineMarker === null) throw new Error('next timeline marker is missing')
    await user.click(nextTimelineMarker)
    await waitFor(() => {
      const selectedTimelineMarker = document.querySelector<HTMLButtonElement>('[data-day-event-id="sentry-start"]')
      expect(selectedTimelineMarker).toHaveFocus()
    })
    expect(screen.getByLabelText('再生位置')).toHaveValue('0')
    const listSelectedVideo = firstCameraVideo()
    fireEvent.canPlay(listSelectedVideo)
    expect(listSelectedVideo.currentTime).toBe(0)

    await user.click(screen.getByRole('button', { name: '次のイベント' }))
    expect(screen.getByLabelText('再生位置')).toHaveValue('65')
    const nextVideo = firstCameraVideo()
    fireEvent.canPlay(nextVideo)
    expect(nextVideo.currentTime).toBe(5)

    await user.click(screen.getByRole('button', { name: '前のイベント' }))
    expect(screen.getByLabelText('再生位置')).toHaveValue('0')
    const previousVideo = firstCameraVideo()
    fireEvent.canPlay(previousVideo)
    expect(previousVideo.currentTime).toBe(0)
    view.unmount()
  })
})
