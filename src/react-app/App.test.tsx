import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useReducer, useRef } from 'react'
import { afterEach, vi } from 'vitest'

import { appReducer, type ReadyState } from './app-state'
import { App } from './App'
import { ReadyWorkspace } from './components/ReadyWorkspace'
import { ScanProgress } from './components/ScanProgress'
import type { LocalFileRef, TeslaCamEvent, TeslaCamLibrary } from '../teslacam/types'

function fileRef(name: string): LocalFileRef {
  return { name, relativePath: `SentryClips/${name}`, size: 4, type: 'video/mp4', getFile: async () => new File(['clip'], name, { type: 'video/mp4' }) }
}

function sentryEvent(
  id: string,
  segments: readonly [string, string],
  detectionTimestamp: string,
): TeslaCamEvent {
  return {
    id, category: 'sentry', timestamp: segments[0],
    metadata: { timestamp: detectionTimestamp },
    segments: segments.map((timestamp) => ({ timestamp, cameras: { front: fileRef(`${id}-${timestamp}-front.mp4`) } })),
  }
}

const preRollEvent = sentryEvent('sentry-preroll', ['2026-08-21_12-00-00', '2026-08-21_12-01-00'], '2026-08-21T12:01:20')
const startEvent = sentryEvent('sentry-start', ['2026-08-21_13-00-00', '2026-08-21_13-01-00'], '2026-08-21T13:00:10')
const selectionLibrary: TeslaCamLibrary = {
  events: [startEvent, preRollEvent],
  eventsByCategory: { recent: [], saved: [], sentry: [startEvent, preRollEvent] },
  warnings: [],
}
const selectionState: ReadyState = {
  kind: 'ready', folderName: 'TeslaCam', library: selectionLibrary,
  filters: { category: 'all', dateKey: null, query: '' },
  selectedEventId: preRollEvent.id,
}

function WorkspaceSelectionHarness() {
  const [state, dispatch] = useReducer(appReducer, selectionState)
  const theaterRef = useRef<HTMLElement>(null)
  if (state.kind !== 'ready') throw new Error('workspace state is not ready')
  return (
    <ReadyWorkspace
      dispatch={dispatch}
      state={state}
      theaterRef={theaterRef}
      onRescan={() => undefined}
      onSelectEvent={(eventId) => dispatch({ kind: 'event-selected', eventId })}
    />
  )
}

function firstCameraVideo(): HTMLVideoElement {
  const video = screen.getAllByTestId('camera-video').find((element): element is HTMLVideoElement => element instanceof HTMLVideoElement)
  if (video === undefined) throw new Error('first camera video is missing')
  return video
}

afterEach(() => vi.restoreAllMocks())

describe('welcome view', () => {
  it('creates fresh paused playback for list and previous-next sentry selections without rewinding a reselected event', async () => {
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
    expect(screen.getByRole('button', { name: '再生' })).toBeVisible()
    expect(firstVideo.currentTime).toBe(5)

    firstVideo.currentTime = 9
    await user.click(screen.getByRole('button', { name: /2026-08-21 12:00:00/ }))
    expect(firstVideo.currentTime).toBe(9)
    await user.click(screen.getByRole('button', { name: 'イベント一覧を開く' }))
    await user.click(screen.getByRole('button', { name: /2026-08-21 13:00:00/ }))
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

  it('shows the folder selection action', () => {
    render(<App />)

    expect(
      screen.getByRole('button', { name: 'TeslaCamフォルダを選ぶ' }),
    ).toBeVisible()
  })

  it('explains that videos stay on this computer', () => {
    render(<App />)

    expect(screen.getByText(/外部へ送信されません/)).toBeVisible()
  })

  it('guides users to a supported desktop browser', () => {
    render(<App />)

    expect(screen.getByText(/ChromeまたはEdge/)).toBeVisible()
  })

  it('announces local scan progress through a live region', () => {
    render(<ScanProgress progress={{ processed: 250, total: 500 }} />)

    expect(screen.getByText('250 / 500 ファイルを確認中です。')).toBeVisible()
    expect(screen.getByText('250 / 500 ファイルを確認中です。').closest('[aria-live]')).toHaveAttribute(
      'aria-live',
      'polite',
    )
  })

  it('announces an indeterminate directory count without a misleading total', () => {
    render(<ScanProgress progress={{ processed: 250, total: 250 }} />)

    expect(screen.getByText('250 ファイルを確認中です。')).toBeVisible()
  })

  it('activates the directory-input fallback when the native picker is unsupported', async () => {
    const user = userEvent.setup()
    const click = vi.spyOn(HTMLInputElement.prototype, 'click')
    Object.defineProperty(window, 'showDirectoryPicker', {
      configurable: true,
      value: undefined,
    })

    render(<App />)

    expect(screen.getByRole('status', { name: 'ブラウザ互換性' })).toHaveTextContent(
      '互換モード',
    )
    await user.click(screen.getByRole('button', { name: 'TeslaCamフォルダを選ぶ' }))

    expect(click).toHaveBeenCalled()
  })

  it('recovers from a denied native picker by using the fallback picker', async () => {
    const user = userEvent.setup()
    const click = vi.spyOn(HTMLInputElement.prototype, 'click')
    Object.defineProperty(window, 'showDirectoryPicker', {
      configurable: true,
      value: async () => Promise.reject(new DOMException('', 'NotAllowedError')),
    })

    render(<App />)
    await user.click(screen.getByRole('button', { name: 'TeslaCamフォルダを選ぶ' }))

    await waitFor(() => {
      expect(screen.getByText('フォルダへのアクセスが許可されませんでした。')).toBeVisible()
    })
    Object.defineProperty(window, 'showDirectoryPicker', {
      configurable: true,
      value: undefined,
    })
    await user.click(screen.getByRole('button', { name: 'もう一度選ぶ' }))

    expect(click).toHaveBeenCalled()
  })

  it('maps an unknown picker rejection to a safe scan failure', async () => {
    const user = userEvent.setup()
    Object.defineProperty(window, 'showDirectoryPicker', {
      configurable: true,
      value: async () => Promise.reject('picker unavailable'),
    })

    render(<App />)
    await user.click(screen.getByRole('button', { name: 'TeslaCamフォルダを選ぶ' }))

    expect(await screen.findByText('フォルダの読み取りを完了できませんでした。')).toBeVisible()
  })

  it('loads a fallback directory after the native picker is unsupported', async () => {
    const user = userEvent.setup()
    const click = vi.spyOn(HTMLInputElement.prototype, 'click')
    Object.defineProperty(window, 'showDirectoryPicker', {
      configurable: true,
      value: undefined,
    })
    render(<App />)
    const fallbackInput = screen.getByLabelText(
      'TeslaCamフォルダを選ぶ（互換モード）',
    )
    const clip = new File(['clip'], '2026-08-20_20-45-00-front.mp4', {
      type: 'video/mp4',
    })
    Object.defineProperty(clip, 'webkitRelativePath', {
      value: 'RecentClips/2026-08-20_20-45-00-front.mp4',
    })

    await user.click(screen.getByRole('button', { name: 'TeslaCamフォルダを選ぶ' }))
    fireEvent.change(fallbackInput, { target: { files: [clip] } })

    await waitFor(() => {
      expect(screen.getByRole('button', { name: '別のフォルダを選ぶ' })).toBeVisible()
    })
    const fallbackClickCount = click.mock.calls.length
    await user.click(screen.getByRole('button', { name: '別のフォルダを選ぶ' }))

    expect(click.mock.calls.length).toBeGreaterThan(fallbackClickCount)
  })

  it('clears the fallback input before a folder is selected again', () => {
    render(<App />)
    const fallbackInput = screen.getByLabelText(
      'TeslaCamフォルダを選ぶ（互換モード）',
    )
    Object.defineProperty(fallbackInput, 'value', {
      configurable: true,
      value: 'previous-folder',
      writable: true,
    })

    fireEvent.click(fallbackInput)

    expect(fallbackInput).toHaveValue('')
  })

  it('shows encrypted-file notice after a local library scan and can recover from invalid input', async () => {
    const user = userEvent.setup()
    Object.defineProperty(window, 'showDirectoryPicker', {
      configurable: true,
      value: async () => ({
        name: 'Private Drive',
        values: async function* () {
          yield {
            kind: 'directory',
            name: 'RecentClips',
            values: async function* () {
              yield {
                kind: 'file',
                name: '2026-08-20_20-45-00-front.mp4',
                getFile: async () => new File(['clip'], '2026-08-20_20-45-00-front.mp4', { type: 'video/mp4' }),
              }
            },
          }
          yield {
            kind: 'directory',
            name: 'EncryptedClips',
            values: async function* () {
              yield {
                kind: 'file',
                name: 'hidden.mp4',
                getFile: async () => new File(['clip'], 'hidden.mp4', { type: 'video/mp4' }),
              }
            },
          }
        },
      }),
    })

    render(<App />)
    await user.click(screen.getByRole('button', { name: 'TeslaCamフォルダを選ぶ' }))

    await waitFor(() => {
      expect(screen.getByLabelText('読み込み元')).toHaveTextContent('Private Drive')
    })

    expect(screen.getByText(/暗号化されたファイル 1 件/)).toBeVisible()
    expect(screen.getByRole('button', { name: 'すべて1' })).toBeVisible()

    const fallbackInput = screen.getByLabelText(
      'TeslaCamフォルダを選ぶ（互換モード）',
    )
    const invalidFile = new File(['clip'], 'clip.mp4')
    Object.defineProperty(invalidFile, 'webkitRelativePath', { value: '' })
    fireEvent.change(fallbackInput, { target: { files: [invalidFile] } })

    await waitFor(() => {
      expect(screen.getByText('TeslaCamフォルダを確認できませんでした。')).toBeVisible()
    })
    await user.click(screen.getByRole('button', { name: 'もう一度選ぶ' }))

    expect(screen.getByRole('button', { name: '別のフォルダを選ぶ' })).toBeVisible()
  })
})
