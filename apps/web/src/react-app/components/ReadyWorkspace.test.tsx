import { fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useReducer } from 'react'
import { afterEach, beforeEach, vi } from 'vitest'

import type { TeslaCamEvent, TeslaCamLibrary } from '../../teslacam/types'
import { mockElementSize } from '../../test/dom-measurement'
import { fileRef, firstCameraVideo, setupTheaterMedia } from '../../test/viewer-fixtures'
import { appReducer, type ReadyState } from '../app-state'
import { ReadyWorkspace } from './ReadyWorkspace'

function sentryEvent(id: string, segments: readonly [string, string], detection: string): TeslaCamEvent {
  return {
    id,
    category: 'sentry',
    timestamp: segments[0],
    metadata: { timestamp: detection },
    segments: segments.map((timestamp) => ({ timestamp, cameras: { front: fileRef(`${id}-${timestamp}-front.mp4`) } })),
  }
}

const preRoll = sentryEvent('sentry-preroll', ['2026-08-21_12-00-00', '2026-08-21_12-01-00'], '2026-08-21T12:01:20')
const start = sentryEvent('sentry-start', ['2026-08-21_13-00-00', '2026-08-21_13-01-00'], '2026-08-21T13:00:10')
const previousDay: TeslaCamEvent = {
  id: 'saved-previous-day',
  category: 'saved',
  timestamp: '2026-08-20_20-00-00',
  segments: [{ timestamp: '2026-08-20_20-00-00', cameras: { front: fileRef('saved-front.mp4') } }],
}
const library: TeslaCamLibrary = {
  events: [start, preRoll, previousDay],
  eventsByCategory: { recent: [], saved: [previousDay], sentry: [start, preRoll] },
  warnings: [],
}

function Harness({ selectedEventId = preRoll.id }: { readonly selectedEventId?: string | null }) {
  const initial: ReadyState = {
    kind: 'ready',
    folderName: 'TeslaCam',
    library,
    filters: { category: 'all', dateKey: null, query: '' },
    selectedEventId,
  }
  const [state, dispatch] = useReducer(appReducer, initial)
  if (state.kind !== 'ready') throw new Error('workspace state is not ready')
  return <ReadyWorkspace band="desktop" dispatch={dispatch} state={state} onRescan={() => undefined} />
}

function rows() {
  return within(screen.getByRole('group', { name: 'イベント一覧' }))
}

beforeEach(setupTheaterMedia)
afterEach(() => {
  vi.restoreAllMocks()
  vi.unstubAllGlobals()
})

describe('workspace selection', () => {
  it('autoplays a Sentry event from 15 s before the detection and never rewinds a reselected event', async () => {
    const user = userEvent.setup()
    render(<Harness />)
    const video = firstCameraVideo(screen.getAllByTestId('camera-video'))
    fireEvent.canPlay(video)

    expect(screen.getByLabelText('再生位置')).toHaveValue('65')
    await waitFor(() => expect(screen.getByRole('button', { name: '一時停止' })).toBeVisible())
    expect(video.currentTime).toBe(5)

    video.currentTime = 9
    await user.click(rows().getByRole('button', { name: '2026-08-21 12:00:00' }))
    expect(firstCameraVideo(screen.getAllByTestId('camera-video'))).toBe(video)
    expect(video.currentTime).toBe(9)
  })

  it('starts the next and previous records at their own initial positions', async () => {
    const user = userEvent.setup()
    render(<Harness />)

    await user.click(screen.getByRole('button', { name: '新しい記録' }))
    expect(rows().getByRole('button', { name: '2026-08-21 13:00:00' })).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByLabelText('再生位置')).toHaveValue('0')
    const video = firstCameraVideo(screen.getAllByTestId('camera-video'))
    fireEvent.canPlay(video)
    expect(video.currentTime).toBe(0)

    await user.click(screen.getByRole('button', { name: '過去の記録' }))
    expect(screen.getByLabelText('再生位置')).toHaveValue('65')
    expect(screen.getByRole('button', { name: '新しい記録' })).toBeEnabled()
  })

  it('moves to the previous recorded day and plays its newest event', async () => {
    const user = userEvent.setup()
    render(<Harness />)

    await user.click(screen.getByRole('button', { name: '前の記録日' }))

    expect(screen.getByLabelText('日付')).toHaveValue('2026-08-20')
    expect(rows().getByRole('button', { name: '2026-08-20 20:00:00' })).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByRole('button', { name: '日付を選ぶ（8月20日 木曜日）' })).toBeVisible()
    expect(screen.getByRole('button', { name: '次の記録日' })).toBeEnabled()
    expect(screen.getByRole('button', { name: '前の記録日' })).toBeDisabled()
    fireEvent.canPlay(firstCameraVideo(screen.getAllByTestId('camera-video')))
    await waitFor(() => expect(screen.getByRole('button', { name: '一時停止' })).toBeVisible())
  })

  it('opens a cluster of nearby records in the rows and selects a member', async () => {
    mockElementSize(332)
    const user = userEvent.setup()
    render(<Harness />)
    const cluster = screen.getByRole('button', { name: '12:00–13:00・2件を開く' })

    await user.click(cluster)
    expect(screen.getByRole('button', { name: '12:00–13:00・2件を閉じる' })).toHaveAttribute('aria-expanded', 'true')
    expect(screen.getByRole('group', { name: '12:00–13:00・2件' })).toBeVisible()
    await user.click(rows().getByRole('button', { name: '2026-08-21 13:00:00' }))

    expect(rows().getByRole('button', { name: '2026-08-21 13:00:00' })).toHaveAttribute('aria-pressed', 'true')
    expect(screen.getByLabelText('再生位置')).toHaveValue('0')
    await user.click(screen.getByRole('button', { name: '絞り込みを解除' }))
    expect(rows().getAllByRole('button')).toHaveLength(2)
  })

  it('filters by category and clears a selection that no longer matches', async () => {
    const user = userEvent.setup()
    render(<Harness />)

    await user.click(screen.getByRole('radio', { name: '保存済み 1件' }))

    expect(screen.getByRole('heading', { name: 'イベントを選択してください' })).toBeVisible()
    expect(rows().getAllByRole('button').map((row) => row.getAttribute('aria-label'))).toEqual(['2026-08-20 20:00:00'])
  })

  it('searches by time or category from the header field', async () => {
    const user = userEvent.setup()
    render(<Harness selectedEventId={null} />)

    await user.click(screen.getByRole('button', { name: '検索を開く' }))
    await user.type(screen.getByRole('searchbox', { name: '検索' }), '13:00')

    expect(rows().getAllByRole('button').map((row) => row.getAttribute('aria-label'))).toEqual(['2026-08-21 13:00:00'])
  })

  it('shows the placeholder when nothing is selected', () => {
    render(<Harness selectedEventId={null} />)

    expect(screen.getByRole('heading', { name: 'イベントを選択してください' })).toBeVisible()
    expect(screen.getByText('一覧から選ぶと、記録された映像をここに表示します。')).toBeVisible()
    expect(screen.queryAllByTestId('camera-video')).toHaveLength(0)
  })
})
