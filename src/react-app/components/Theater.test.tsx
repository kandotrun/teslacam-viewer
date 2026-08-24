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

function firstCameraVideo(): HTMLVideoElement {
  const video = screen.getAllByTestId('camera-video').find(
    (element): element is HTMLVideoElement => element instanceof HTMLVideoElement,
  )
  if (video === undefined) throw new Error('first camera video is missing')
  return video
}

function sentryEventWithCameras(id: string): TeslaCamEvent {
  const baseEvent = eventWithCameras()
  const firstSegment = baseEvent.segments.at(0)
  if (firstSegment === undefined) throw new Error('camera event is missing its first segment')
  return {
    ...baseEvent,
    id,
    category: 'sentry',
    metadata: { timestamp: '2026-08-21T12:01:20' },
    segments: [
      firstSegment,
      { ...firstSegment, timestamp: '2026-08-21_12-01-00' },
    ],
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
  it('opens sentry playback paused at its pre-roll location', () => {
    render(<Theater event={sentryEventWithCameras('sentry-preroll')} onPrevious={() => undefined} onNext={() => undefined} hasPrevious hasNext />)

    expect(screen.getByLabelText('再生位置')).toHaveValue('65')
    expect(screen.getByRole('button', { name: '再生' })).toBeVisible()
    const firstVideo = firstCameraVideo()
    fireEvent.canPlay(firstVideo)

    expect(firstVideo.currentTime).toBe(5)
  })

  it('keeps the selected evidence context next to the playback surface', () => {
    render(
      <Theater
        event={eventWithCameras()}
        hasNext
        hasPrevious
        sourceName="TeslaCam"
        onNext={() => undefined}
        onPrevious={() => undefined}
      />,
    )

    const context = screen.getByLabelText('再生中のイベント')

    expect(context).toHaveTextContent('TeslaCam')
    expect(context).toHaveTextContent('2026-08-21 12:00:00')
    expect(context).toHaveTextContent('保存済み')
    expect(context).toHaveTextContent('6/6 カメラ')
    expect(context).toHaveTextContent('1分')
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
    expect(screen.getByLabelText('代表映像')).toHaveTextContent('6カメラ同期映像ではありません。')
  })

  it('maps an unknown getFile rejection to an unavailable tile', async () => {
    const failing = fileRef('front.mp4', async () => Promise.reject('unavailable'))
    const event: TeslaCamEvent = {
      ...eventWithCameras([]),
      segments: [{ timestamp: '2026-08-21_12-00-00', cameras: { front: failing } }],
    }

    render(<Theater event={event} onPrevious={() => undefined} onNext={() => undefined} hasPrevious={false} hasNext={false} />)

    expect(await screen.findByText('映像を読み込めません')).toBeVisible()
  })

  it('maps an unknown representative getFile rejection to a safe fallback state', async () => {
    const event: TeslaCamEvent = {
      ...eventWithCameras([]),
      representativeFile: fileRef('event.mp4', async () => Promise.reject('unavailable')),
    }

    render(<Theater event={event} onPrevious={() => undefined} onNext={() => undefined} hasPrevious={false} hasNext={false} />)

    expect(await screen.findByText('代表映像を読み込めません。')).toBeVisible()
    expect(document.querySelector('.representative-video video')).not.toBeInTheDocument()
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
