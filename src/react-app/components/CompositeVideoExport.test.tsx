import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, vi } from 'vitest'

import { eventWithCameras } from './Theater.test-fixtures'
import { CompositeVideoExport } from './CompositeVideoExport'

const exportMocks = vi.hoisted(() => ({
  recordingFormat: vi.fn(() => ({
    mimeType: 'video/webm;codecs=vp9',
    extension: 'webm',
    label: 'WebM',
  })),
  exportVideo: vi.fn(),
}))

vi.mock('../export/composite-video', () => ({
  compositeRecordingFormat: exportMocks.recordingFormat,
  exportCompositeVideo: exportMocks.exportVideo,
}))

afterEach(() => {
  vi.clearAllMocks()
})

describe('CompositeVideoExport', () => {
  it('explains the local real-time export before starting', async () => {
    const user = userEvent.setup()
    render(<CompositeVideoExport event={eventWithCameras()} />)

    await user.click(screen.getByRole('button', { name: '合成動画を書き出す' }))

    expect(screen.getByRole('dialog', { name: '合成動画を書き出す' })).toBeVisible()
    expect(screen.getByText('映像はこのMacの中だけで処理され、外部へ送信されません。')).toBeVisible()
    expect(screen.getByText('1280 × 720')).toBeVisible()
    expect(screen.getByText('WebM')).toBeVisible()
    expect(screen.getByText(/録画時間と同程度/)).toBeVisible()
  })

  it('reports progress and offers the completed video for saving', async () => {
    const user = userEvent.setup()
    exportMocks.exportVideo.mockImplementationOnce(async ({ onProgress }) => {
      onProgress?.(0.5)
      return {
        blob: new Blob(['video'], { type: 'video/webm' }),
        mimeType: 'video/webm',
        extension: 'webm',
      }
    })
    render(<CompositeVideoExport event={eventWithCameras()} />)

    await user.click(screen.getByRole('button', { name: '合成動画を書き出す' }))
    await user.click(screen.getByRole('button', { name: '書き出しを開始' }))

    await waitFor(() => expect(screen.getByText('書き出し完了 · WEBM · 1280 × 720')).toBeVisible())
    expect(screen.getByRole('button', { name: 'WEBMを保存' })).toBeVisible()
    expect(exportMocks.exportVideo).toHaveBeenCalledWith(expect.objectContaining({
      event: expect.objectContaining({ id: 'event' }),
      signal: expect.any(AbortSignal),
      onProgress: expect.any(Function),
    }))
  })
})
