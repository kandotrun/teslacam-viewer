import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { afterEach, beforeEach, vi } from 'vitest'

import { cameraIds, type TeslaCamEvent } from '../../../teslacam/types'
import { eventWithCameras, fileRef, firstCameraVideo, labels, sentryEventWithCameras, setupTheaterMedia } from '../../../test/viewer-fixtures'
import type { LayoutBand, ViewMode } from '../../presentation/stage-layout'
import { ViewerSession } from './ViewerSession'

type HarnessProps = {
  readonly event: TeslaCamEvent
  readonly band?: LayoutBand
  readonly onOlder?: (() => void) | null
  readonly onNewer?: (() => void) | null
}

function Viewer({ event, band = 'desktop', onOlder = null, onNewer = null }: HarnessProps) {
  const [viewMode, setViewMode] = useState<ViewMode>('spatial')
  return <ViewerSession band={band} event={event} viewMode={viewMode} onNewer={onNewer} onOlder={onOlder} onViewMode={setViewMode} />
}

function sentryWith(detection: string, segments: readonly string[] = ['2026-08-21_12-00-00']): TeslaCamEvent {
  const base = eventWithCameras()
  const cameras = base.segments[0]?.cameras ?? {}
  return {
    ...base,
    category: 'sentry',
    metadata: { timestamp: detection, reason: 'sentry_aware_object_detection', city: 'Sample city', street: 'Sample street', est_lat: '35.0000', est_lon: '139.0000' },
    segments: segments.map((timestamp) => ({ timestamp, cameras })),
  }
}

beforeEach(setupTheaterMedia)
afterEach(() => vi.restoreAllMocks())

describe('stage', () => {
  it('keeps six camera windows in canonical order around the car', () => {
    render(<Viewer event={eventWithCameras()} />)
    const windows = screen.getAllByRole('button', { name: /カメラを拡大$/u })

    expect(windows.map((window) => window.getAttribute('aria-label'))).toEqual(labels.map((label) => `${label}カメラを拡大`))
    expect(windows.map((window) => window.getAttribute('data-camera'))).toEqual(cameraIds)
    expect(screen.getByRole('main', { name: '6台のカメラ' })).toHaveAttribute('data-mode', 'spatial')
  })

  it('switches between 車両配置 and 6分割 with the same six videos and no reloading', async () => {
    const user = userEvent.setup()
    render(<Viewer event={eventWithCameras()} />)
    await waitFor(() => expect(URL.createObjectURL).toHaveBeenCalledTimes(6))
    const videos = screen.getAllByTestId('camera-video')
    const loads = vi.mocked(HTMLMediaElement.prototype.load).mock.calls.length

    expect(screen.getByRole('radio', { name: '車両配置' })).toBeChecked()
    await user.click(screen.getByRole('radio', { name: '6分割' }))
    expect(screen.getByRole('main', { name: '6台のカメラ' })).toHaveAttribute('data-mode', 'grid')
    await user.click(screen.getByRole('radio', { name: '車両配置' }))

    expect(screen.getAllByTestId('camera-video')).toEqual(videos)
    expect(URL.createObjectURL).toHaveBeenCalledTimes(6)
    expect(URL.revokeObjectURL).not.toHaveBeenCalled()
    expect(vi.mocked(HTMLMediaElement.prototype.load).mock.calls.length).toBe(loads)
  })

  it('expands a camera without reloading any of the six videos', async () => {
    const user = userEvent.setup()
    render(<Viewer event={eventWithCameras()} />)
    await waitFor(() => expect(URL.createObjectURL).toHaveBeenCalledTimes(6))
    const loads = vi.mocked(HTMLMediaElement.prototype.load).mock.calls.length

    await user.click(screen.getByRole('button', { name: '左ピラーカメラを拡大' }))

    expect(screen.getByRole('button', { name: '左ピラーカメラをグリッドへ戻す' })).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('main', { name: '6台のカメラ' })).toHaveAttribute('data-mode', 'expanded')
    expect(screen.getByRole('group', { name: '表示するカメラ' })).toBeVisible()
    expect(URL.createObjectURL).toHaveBeenCalledTimes(6)
    expect(vi.mocked(HTMLMediaElement.prototype.load).mock.calls.length).toBe(loads)
    await user.click(screen.getByRole('button', { name: '右リピーターを表示' }))
    expect(screen.getByRole('button', { name: '右リピーターカメラをグリッドへ戻す' })).toBeVisible()
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(screen.getByRole('main', { name: '6台のカメラ' })).toHaveAttribute('data-mode', 'spatial')
  })

  it('shows missing and unreadable cameras and the representative video once every camera failed', async () => {
    const failing = fileRef('front.mp4', async () => Promise.reject(new DOMException('', 'NotReadableError')))
    const event: TeslaCamEvent = {
      ...eventWithCameras([]),
      segments: [{ timestamp: '2026-08-21_12-00-00', cameras: { front: failing } }],
      representativeFile: fileRef('event.mp4'),
    }
    render(<Viewer event={event} />)

    expect(screen.getAllByText('記録がありません')).toHaveLength(5)
    expect(screen.getByRole('button', { name: 'リアカメラを拡大' })).toHaveAccessibleDescription('記録がありません')
    await waitFor(() => expect(screen.getByText('映像を読み込めません')).toBeVisible())
    expect(screen.getByRole('button', { name: 'フロントカメラを拡大' })).toHaveAccessibleDescription('映像を読み込めません')
    expect(screen.getByRole('main', { name: '6台のカメラ' })).toHaveAttribute('data-mode', 'representative')
    expect(screen.getByRole('radio', { name: '代表映像' })).toBeChecked()
    expect(screen.getByRole('radio', { name: '車両配置' })).toBeDisabled()
    expect(screen.getByRole('region', { name: '代表映像' })).toHaveTextContent('代表映像（単独再生）6カメラ同期映像ではありません。')
  })

  it('says when event.mp4 cannot be read', async () => {
    const event: TeslaCamEvent = { ...eventWithCameras([]), representativeFile: fileRef('event.mp4', async () => Promise.reject('unavailable')) }
    render(<Viewer event={event} />)

    expect(await screen.findByText('代表映像を読み込めません。')).toBeVisible()
    expect(document.querySelector('.representative video')).not.toBeInTheDocument()
  })

  it('does not acquire a media URL when a file resolves after the session ended', async () => {
    let resolveFile: (file: File) => void = () => undefined
    const pending = new Promise<File>((resolve) => {
      resolveFile = resolve
    })
    const base = eventWithCameras([])
    const event: TeslaCamEvent = { ...base, segments: [{ timestamp: base.timestamp, cameras: { front: fileRef('front.mp4', async () => pending) } }] }
    const view = render(<Viewer event={event} />)
    view.unmount()
    resolveFile(new File(['clip'], 'front.mp4', { type: 'video/mp4' }))
    await Promise.resolve()

    expect(URL.createObjectURL).not.toHaveBeenCalled()
  })

  it('shows one large camera with the mini car switcher on phones', async () => {
    const user = userEvent.setup()
    render(<Viewer band="phone" event={eventWithCameras()} />)
    const stage = screen.getByRole('main', { name: '6台のカメラ' })

    expect(stage).toHaveAttribute('data-mode', 'single')
    expect(screen.getAllByTestId('camera-video')).toHaveLength(6)
    expect(stage.querySelectorAll('.camera-slot:not([data-area="hidden"])')).toHaveLength(1)
    await user.click(screen.getByRole('button', { name: '右ピラーを表示' }))
    expect(screen.getByRole('button', { name: '右ピラーを表示' })).toHaveAttribute('aria-pressed', 'true')
    expect(stage.querySelector('.camera-slot[data-area="main"]')).toHaveAttribute('data-camera', 'right_pillar')
  })
})

describe('transport and shortcuts', () => {
  it('starts a Sentry event 15 s before the detection and marks both on the scrubber', async () => {
    render(<Viewer event={sentryEventWithCameras('sentry-preroll')} />)
    const video = firstCameraVideo(screen.getAllByTestId('camera-video'))
    fireEvent.canPlay(video)

    expect(screen.getByLabelText('再生位置')).toHaveValue('65')
    expect(screen.getByLabelText('再生位置')).toHaveAttribute('aria-valuetext', '1分5秒 / 2分')
    await waitFor(() => expect(screen.getByRole('button', { name: '一時停止' })).toBeVisible())
    expect(video.currentTime).toBe(5)
    expect(screen.getByRole('img', { name: '検知位置' }).getAttribute('style')).toContain('66.66')
    expect(screen.getByText('検知の15秒前')).toBeVisible()
  })

  it('starts at the beginning without a pre-roll mark when the detection is earlier than 15 s', () => {
    render(<Viewer event={sentryWith('2026-08-21T12:00:10')} />)

    expect(screen.getByLabelText('再生位置')).toHaveValue('0')
    expect(screen.getByRole('img', { name: '検知位置' })).toBeInTheDocument()
    expect(screen.queryByText('検知の15秒前')).not.toBeInTheDocument()
  })

  it('never marks a detection for saved or recent events', () => {
    render(<Viewer event={eventWithCameras()} />)

    expect(screen.queryByRole('img', { name: '検知位置' })).not.toBeInTheDocument()
  })

  it('offers older and newer records, ±10 s and playback speeds', async () => {
    const older = vi.fn()
    const user = userEvent.setup()
    render(<Viewer event={eventWithCameras()} onOlder={older} />)

    expect(screen.getByRole('button', { name: '新しい記録' })).toBeDisabled()
    await user.click(screen.getByRole('button', { name: '過去の記録' }))
    expect(older).toHaveBeenCalledTimes(1)
    await user.click(screen.getByRole('button', { name: '10秒進む' }))
    expect(screen.getByLabelText('再生位置')).toHaveValue('10')
    const rate = screen.getByRole('combobox', { name: '再生速度' })
    expect(rate).toHaveValue('1')
    await user.selectOptions(rate, '1.5')
    expect(rate).toHaveValue('1.5')
  })

  it('handles digits, Escape, Space, arrows and [ ] outside controls', async () => {
    const older = vi.fn()
    const newer = vi.fn()
    render(<Viewer event={eventWithCameras()} onNewer={newer} onOlder={older} />)

    fireEvent.keyDown(window, { key: '6' })
    expect(screen.getByRole('button', { name: '右リピーターカメラをグリッドへ戻す' })).toHaveAttribute('aria-pressed', 'true')
    fireEvent.keyDown(window, { key: 'Escape' })
    expect(screen.getByRole('button', { name: '右リピーターカメラを拡大' })).toHaveAttribute('aria-pressed', 'false')
    for (const video of screen.getAllByTestId('camera-video')) fireEvent.canPlay(video)
    await waitFor(() => expect(screen.getByRole('button', { name: '一時停止' })).toBeVisible())
    fireEvent.keyDown(window, { key: ' ' })
    expect(screen.getByRole('button', { name: '再生' })).toBeVisible()
    fireEvent.keyDown(window, { key: 'ArrowRight' })
    expect(screen.getByLabelText('再生位置')).toHaveValue('10')
    fireEvent.keyDown(window, { key: '[' })
    fireEvent.keyDown(window, { key: ']' })
    expect(older).toHaveBeenCalledTimes(1)
    expect(newer).toHaveBeenCalledTimes(1)
  })

  it('leaves keys to a focused button but still collapses with Escape', async () => {
    const user = userEvent.setup()
    render(<Viewer event={eventWithCameras()} />)
    const front = screen.getByRole('button', { name: 'フロントカメラを拡大' })
    await user.click(front)
    const expanded = screen.getByRole('button', { name: 'フロントカメラをグリッドへ戻す' })
    expect(expanded).toHaveFocus()

    fireEvent.keyDown(expanded, { key: 'ArrowRight' })
    fireEvent.keyDown(expanded, { key: '6' })
    expect(screen.getByLabelText('再生位置')).toHaveValue('0')
    expect(screen.queryByRole('button', { name: '右リピーターカメラをグリッドへ戻す' })).not.toBeInTheDocument()
    fireEvent.keyDown(expanded, { key: 'Escape' })
    expect(screen.getByRole('button', { name: 'フロントカメラを拡大' })).toHaveAttribute('aria-pressed', 'false')
  })

  it('keeps the no-playable-camera message when play is pressed before any camera can play', async () => {
    const user = userEvent.setup()
    const event: TeslaCamEvent = { ...eventWithCameras([]), segments: [{ timestamp: '2026-08-21_12-00-00', cameras: {} }] }
    render(<Viewer event={event} />)
    await waitFor(() => expect(screen.getAllByText('記録がありません')).toHaveLength(6))

    await user.click(screen.getByRole('button', { name: '再生' }))

    expect(screen.getByRole('status').textContent).toBe('再生できるカメラ映像がありません。\n読み込み後に再試行してください。')
  })
})

describe('event pill and details', () => {
  it('summarises the event and opens every available detail without external links', async () => {
    const user = userEvent.setup()
    render(<Viewer event={sentryWith('2026-08-21T12:00:30')} />)
    const pill = within(screen.getByRole('region', { name: '再生中のイベント' })).getByRole('button', { expanded: false })

    expect(pill).toHaveTextContent('セントリー12:00 開始Sample city Sample street物体の接近を12:00:30に検知')
    expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('セントリー 2026-08-21 12:00:00')
    await user.click(pill)
    const details = screen.getByRole('dialog', { name: 'イベント詳細' })
    const text = details.textContent ?? ''
    for (const expected of ['場所（Teslaの推定値）Sample city Sample street', '推定座標35.0000, 139.0000', '検知物体の接近を12:00:30に検知', '再生開始12:00:15（検知の15秒前）', 'カメラ6カメラ', '長さ1分', 'セグメント1セグメント', '録画開始2026-08-21 12:00:00']) {
      expect(text).toContain(expected)
    }
    expect(within(details).queryByRole('link')).not.toBeInTheDocument()
    const technical = within(details).getByText('技術情報').closest('details')
    expect(technical).not.toHaveAttribute('open')
    fireEvent.keyDown(details, { key: 'Escape' })
    expect(screen.queryByRole('dialog', { name: 'イベント詳細' })).not.toBeInTheDocument()
    expect(pill).toHaveFocus()
  })

  it('names missing cameras and leaves out location rows without data', async () => {
    const user = userEvent.setup()
    render(<Viewer event={eventWithCameras(['front', 'back', 'left_pillar', 'left_repeater', 'right_pillar'])} />)
    await user.click(within(screen.getByRole('region', { name: '再生中のイベント' })).getByRole('button', { expanded: false }))
    const details = screen.getByRole('dialog', { name: 'イベント詳細' })

    expect(details).toHaveTextContent('5カメラ右リピーターの記録がありません')
    expect(details).not.toHaveTextContent('場所（Teslaの推定値）')
    expect(details).not.toHaveTextContent('検知')
  })
})

function eventWithTwoSegments(): TeslaCamEvent {
  const segment = (timestamp: string): TeslaCamEvent['segments'][number] => {
    const cameras: Partial<Record<(typeof cameraIds)[number], ReturnType<typeof fileRef>>> = {}
    for (const cameraId of cameraIds) cameras[cameraId] = fileRef(`${timestamp}-${cameraId}.mp4`)
    return { timestamp, cameras }
  }
  return {
    ...eventWithCameras(),
    id: 'two-segments',
    segments: [segment('2026-08-21_12-00-00'), segment('2026-08-21_12-01-00')],
  }
}

describe('segments', () => {
  it('preloads the next segment in hidden videos and plays them at the boundary without loading anything', async () => {
    render(<Viewer event={eventWithTwoSegments()} />)
    await waitFor(() => expect(URL.createObjectURL).toHaveBeenCalledTimes(12))
    const playing = screen.getAllByTestId('camera-video')
    const preloaded = [...document.querySelectorAll<HTMLVideoElement>('video[data-buffer="spare"]')]
    expect(playing).toHaveLength(6)
    expect(preloaded).toHaveLength(6)
    for (const video of [...playing, ...preloaded]) {
      Object.defineProperty(video, 'readyState', { configurable: true, value: HTMLMediaElement.HAVE_ENOUGH_DATA })
      fireEvent.canPlay(video)
    }
    const loads = vi.mocked(HTMLMediaElement.prototype.load).mock.calls.length
    vi.mocked(HTMLMediaElement.prototype.play).mockClear()

    const source = firstCameraVideo(playing)
    // jsdom does not play media, so mark the source as finished the way a browser does before `ended`.
    Object.defineProperty(source, 'ended', { configurable: true, value: true })
    fireEvent.ended(source)

    await waitFor(() => expect(screen.getAllByTestId('camera-video')).toEqual(preloaded))
    expect(preloaded.map((video) => video.dataset['buffer'])).toEqual(Array.from({ length: 6 }, () => 'active'))
    expect(URL.createObjectURL).toHaveBeenCalledTimes(12)
    expect(vi.mocked(HTMLMediaElement.prototype.load).mock.calls.length).toBe(loads)
    await waitFor(() => expect(vi.mocked(HTMLMediaElement.prototype.play).mock.contexts).toEqual(expect.arrayContaining(preloaded)))
    expect(screen.getByRole('button', { name: '一時停止' })).toBeVisible()
  })
})
