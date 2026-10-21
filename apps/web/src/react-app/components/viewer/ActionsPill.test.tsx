import { render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, vi } from 'vitest'

import type { TeslaCamEvent } from '../../../teslacam/types'
import { eventWithCameras, fileRef } from '../../../test/viewer-fixtures'
import { ActionsPill } from './ActionsPill'
import { useCompositeExport } from './use-composite-export'

const exportMocks = vi.hoisted(() => ({
  potentialSupport: vi.fn(() => true),
  resolveFormat: vi.fn(async () => ({ mimeType: 'video/mp4', extension: 'mp4', label: 'MP4', fallbackLabel: 'WebM' })),
  exportVideo: vi.fn(),
}))

vi.mock('../../export/composite-video', () => ({
  compositeExportPotentiallySupported: exportMocks.potentialSupport,
  exportCompositeVideo: exportMocks.exportVideo,
  resolveCompositeRecordingFormat: exportMocks.resolveFormat,
}))

function Actions({ event }: { readonly event: TeslaCamEvent }) {
  const exporter = useCompositeExport(event)
  return <ActionsPill event={event} exporter={exporter} mode="spatial" modes={['spatial', 'grid']} representativeForced={false} onMode={() => undefined} />
}

afterEach(() => vi.clearAllMocks())

describe('export control', () => {
  it('shows the frame, size and format before starting', async () => {
    const user = userEvent.setup()
    render(<Actions event={eventWithCameras()} />)

    await user.click(screen.getByRole('button', { name: '合成動画を書き出す' }))

    const dialog = await screen.findByRole('dialog', { name: '合成動画を書き出す' })
    expect(dialog).toHaveTextContent('1280 × 720')
    expect(dialog).toHaveTextContent('MP4（失敗時はWebM）')
    expect(dialog).toHaveTextContent('イベント全体・1分')
    expect(dialog).toHaveTextContent('対応ブラウザでは高速に書き出します。完了まではタブを閉じないでください。')
    expect(dialog.querySelector('svg.export-preview')).toBeInTheDocument()
  })

  it('shows progress while exporting and then offers the video for saving', async () => {
    const user = userEvent.setup()
    let finish: () => void = () => undefined
    exportMocks.exportVideo.mockImplementationOnce(async ({ onProgress }: { readonly onProgress?: (progress: number) => void }) => {
      onProgress?.(0.42)
      await new Promise<void>((resolve) => {
        finish = resolve
      })
      return { blob: new Blob(['video'], { type: 'video/mp4' }), mimeType: 'video/mp4', extension: 'mp4' }
    })
    render(<Actions event={eventWithCameras()} />)

    await user.click(screen.getByRole('button', { name: '合成動画を書き出す' }))
    await user.click(await screen.findByRole('button', { name: '書き出しを開始' }))
    await waitFor(() => expect(screen.getByRole('button', { name: '書き出し中 42%' })).toBeVisible())
    await user.click(screen.getByRole('button', { name: '書き出し中 42%' }))
    expect(screen.getByText('00:25 / 01:00')).toBeVisible()
    finish()

    await waitFor(() => expect(screen.getByRole('status')).toHaveTextContent('書き出し完了'))
    expect(screen.getByRole('button', { name: 'MP4を保存' })).toBeVisible()
  })

  it('aborts on 中止 and says so', async () => {
    const user = userEvent.setup()
    exportMocks.exportVideo.mockImplementationOnce(async ({ signal }: { readonly signal: AbortSignal }) => new Promise((_, reject) => {
      signal.addEventListener('abort', () => reject(new Error('aborted')))
    }))
    render(<Actions event={eventWithCameras()} />)

    await user.click(screen.getByRole('button', { name: '合成動画を書き出す' }))
    await user.click(await screen.findByRole('button', { name: '書き出しを開始' }))
    await user.click(await screen.findByRole('button', { name: /^書き出し中/u }))
    await user.click(screen.getByRole('button', { name: '中止' }))

    expect(screen.getByRole('status')).toHaveTextContent('書き出しを中止しました。')
    expect(screen.queryByRole('button', { name: /を保存$/u })).not.toBeInTheDocument()
  })

  it('returns focus to the trigger when the dialog is cancelled', async () => {
    const user = userEvent.setup()
    render(<Actions event={eventWithCameras()} />)
    const trigger = screen.getByRole('button', { name: '合成動画を書き出す' })

    await user.click(trigger)
    await user.click(await screen.findByRole('button', { name: 'キャンセル' }))

    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()
    await waitFor(() => expect(screen.getByRole('button', { name: '合成動画を書き出す' })).toHaveFocus())
  })

  it('explains why an event without a camera in every segment cannot be exported', async () => {
    const user = userEvent.setup()
    const event: TeslaCamEvent = {
      ...eventWithCameras(),
      segments: [
        { timestamp: '2026-08-21_12-00-00', cameras: { front: fileRef('front.mp4') } },
        { timestamp: '2026-08-21_12-01-00', cameras: { back: fileRef('back.mp4') } },
      ],
    }
    render(<Actions event={event} />)
    const trigger = screen.getByRole('button', { name: '合成動画を書き出す' })

    expect(trigger).toHaveAttribute('aria-disabled', 'true')
    expect(trigger).toHaveAccessibleDescription('6カメラ映像がないため書き出せません')
    await user.click(trigger)
    expect(exportMocks.resolveFormat).not.toHaveBeenCalled()
    expect(screen.getByText('6カメラ映像がないため書き出せません')).toBeVisible()
  })
})
