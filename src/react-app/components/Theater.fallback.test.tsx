import { render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, vi } from 'vitest'

import type { TeslaCamEvent } from '../../teslacam/types'
import { Theater } from './Theater.test-harness'
import { eventWithCameras, fileRef, setupTheaterMedia } from './Theater.test-fixtures'

beforeEach(setupTheaterMedia)
afterEach(() => vi.restoreAllMocks())

describe('Theater media fallbacks', () => {
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
    expect(screen.queryByRole('group', { name: '表示レイアウト' })).not.toBeInTheDocument()
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
      segments: [{ timestamp: baseEvent.timestamp, cameras: { front: fileRef('front.mp4', async () => pendingFile) } }],
    }
    const view = render(<Theater event={pendingEvent} onPrevious={() => undefined} onNext={() => undefined} hasPrevious={false} hasNext={false} />)
    view.unmount()
    resolveFile(new File(['clip'], 'front.mp4', { type: 'video/mp4' }))
    await Promise.resolve()
    expect(URL.createObjectURL).not.toHaveBeenCalled()
  })
})
