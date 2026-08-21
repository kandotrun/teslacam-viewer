import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { vi } from 'vitest'

import { cameraIds, type CameraId, type LocalFileRef, type TeslaCamEvent } from '../../teslacam/types'
import { Theater } from './Theater'

const labels = ['フロント', 'リア', '左ピラー', '左リピーター', '右ピラー', '右リピーター'] as const

function fileRef(name: string, getFile = async () => new File(['clip'], name, { type: 'video/mp4' })): LocalFileRef {
  return { name, relativePath: `SavedClips/event/${name}`, size: 4, type: 'video/mp4', getFile }
}

function eventWithCameras(cameras: readonly CameraId[] = cameraIds): TeslaCamEvent {
  const refs: Partial<Record<CameraId, LocalFileRef>> = {}
  for (const cameraId of cameras) refs[cameraId] = fileRef(`${cameraId}.mp4`)
  return {
    id: 'event',
    category: 'saved',
    timestamp: '2026-08-21_12-00-00',
    metadata: { timestamp: '2026-08-21T12:00:30' },
    segments: [{ timestamp: '2026-08-21_12-00-00', cameras: refs }],
  }
}

beforeEach(() => {
  vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:camera')
  vi.spyOn(URL, 'revokeObjectURL').mockImplementation(() => undefined)
  HTMLMediaElement.prototype.load = vi.fn()
  HTMLMediaElement.prototype.pause = vi.fn()
  HTMLMediaElement.prototype.play = vi.fn(async () => undefined)
})

afterEach(() => vi.restoreAllMocks())

describe('Theater', () => {
  it('renders six cameras in canonical Japanese order and keeps every tile mounted while expanded', async () => {
    const user = userEvent.setup()
    render(<Theater event={eventWithCameras()} onPrevious={() => undefined} onNext={() => undefined} hasPrevious hasNext />)

    const tiles = screen.getAllByRole('button', { name: /カメラを拡大/ })
    expect(tiles.map((tile) => tile.getAttribute('aria-label'))).toEqual(labels.map((label) => `${label}カメラを拡大`))
    const leftPillar = tiles.at(2)
    if (leftPillar === undefined) throw new Error('left pillar tile is missing')
    await user.click(leftPillar)
    expect(screen.getAllByTestId('camera-video')).toHaveLength(6)
    expect(leftPillar).toHaveAttribute('aria-pressed', 'true')
    await user.click(leftPillar)
    expect(leftPillar).toHaveAttribute('aria-pressed', 'false')
  })

  it('supports digit expansion, Escape grid, Space playback, and arrow skipping outside editable controls', async () => {
    render(<Theater event={eventWithCameras()} onPrevious={() => undefined} onNext={() => undefined} hasPrevious hasNext />)

    fireEvent.keyDown(window, { key: '6' })
    expect(screen.getByRole('button', { name: '右リピーターカメラをグリッドへ戻す' })).toHaveAttribute('aria-pressed', 'true')
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(screen.getByRole('button', { name: '右リピーターカメラを拡大' })).toHaveAttribute('aria-pressed', 'false')
    for (const videoElement of screen.getAllByTestId('camera-video')) {
      fireEvent.canPlay(videoElement)
    }
    fireEvent.keyDown(window, { key: ' ' })
    await waitFor(() => expect(screen.getByRole('button', { name: '一時停止' })).toBeVisible())
    fireEvent.keyDown(window, { key: 'ArrowRight' })
    expect(screen.getByLabelText('再生位置')).toHaveValue('10')
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
    expect(screen.getByLabelText('再生速度')).toHaveTextContent('0.5×1×1.5×2×')
    expect(screen.getByLabelText('検知位置')).toHaveAttribute('style', expect.stringContaining('50%'))
  })

  it('omits invalid or out-of-range detection metadata and ignores shortcuts from controls', () => {
    const outsideEvent: TeslaCamEvent = {
      ...eventWithCameras(),
      metadata: { timestamp: '2026-08-21T12:02:00' },
    }
    render(<Theater event={outsideEvent} onPrevious={() => undefined} onNext={() => undefined} hasPrevious hasNext />)
    const playButton = screen.getByRole('button', { name: '再生' })

    fireEvent.keyDown(playButton, { key: 'ArrowRight' })

    expect(screen.queryByLabelText('検知位置')).not.toBeInTheDocument()
    expect(screen.getByLabelText('再生位置')).toHaveValue('0')
  })

  it('keeps explicit missing and unavailable tile summaries and shows representative fallback only after all streams terminate', async () => {
    const failing = fileRef('front.mp4', async () => Promise.reject(new DOMException('', 'NotReadableError')))
    const event: TeslaCamEvent = {
      ...eventWithCameras([]),
      segments: [{ timestamp: '2026-08-21_12-00-00', cameras: { front: failing } }],
      representativeFile: fileRef('event.mp4'),
    }
    render(<Theater event={event} onPrevious={() => undefined} onNext={() => undefined} hasPrevious={false} hasNext={false} />)

    expect(screen.getAllByText('映像がありません')).toHaveLength(5)
    await waitFor(() => expect(screen.getByText('映像を読み込めません')).toBeVisible())
    expect(screen.getByText('代表映像（単独再生）')).toBeVisible()
    expect(screen.getByText(/6カメラ同期映像ではありません/)).toBeVisible()
  })

  it('does not acquire a Blob URL when file resolution becomes stale after unmount', async () => {
    let resolveFile: (file: File) => void = () => undefined
    const pendingFile = new Promise<File>((resolve) => {
      resolveFile = resolve
    })
    const baseEvent = eventWithCameras([])
    const pendingEvent: TeslaCamEvent = {
      ...baseEvent,
      segments: [{
        timestamp: baseEvent.timestamp,
        cameras: { front: fileRef('front.mp4', async () => pendingFile) },
      }],
    }
    const view = render(<Theater event={pendingEvent} onPrevious={() => undefined} onNext={() => undefined} hasPrevious={false} hasNext={false} />)

    view.unmount()
    resolveFile(new File(['clip'], 'front.mp4', { type: 'video/mp4' }))
    await Promise.resolve()

    expect(URL.createObjectURL).not.toHaveBeenCalled()
  })
})
