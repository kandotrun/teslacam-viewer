import { Download } from 'lucide-react'
import { useEffect, useReducer, useRef } from 'react'

import type { TeslaCamEvent } from '../../teslacam/types'
import {
  compositeExportPotentiallySupported,
  exportCompositeVideo,
  resolveCompositeRecordingFormat,
  type CompositeRecordingFormat,
} from '../export/composite-video'
import { cameraAvailability } from '../event-summary'
import { eventDuration } from '../playback/timeline'
import { CompositeExportDialog } from './CompositeExportDialog'

type CompositeVideoExportProps = {
  readonly event: TeslaCamEvent
}

type ExportResult = Awaited<ReturnType<typeof exportCompositeVideo>>
type ExportState =
  | { readonly status: 'idle'; readonly dialogOpen: boolean; readonly format: CompositeRecordingFormat | null; readonly notice: string | null }
  | { readonly status: 'exporting'; readonly progress: number }
  | { readonly status: 'complete'; readonly result: ExportResult }
  | { readonly status: 'error'; readonly error: string }

type ExportAction =
  | { readonly kind: 'dialog-opened'; readonly format: CompositeRecordingFormat }
  | { readonly kind: 'dialog-closed' }
  | { readonly kind: 'started' }
  | { readonly kind: 'progressed'; readonly progress: number }
  | { readonly kind: 'completed'; readonly result: ExportResult }
  | { readonly kind: 'failed'; readonly error: string }
  | { readonly kind: 'cancelled' }
  | { readonly kind: 'reset' }

const initialExportState: ExportState = {
  status: 'idle',
  dialogOpen: false,
  format: null,
  notice: null,
}

function exportReducer(state: ExportState, action: ExportAction): ExportState {
  switch (action.kind) {
    case 'dialog-opened':
      return { status: 'idle', dialogOpen: true, format: action.format, notice: null }
    case 'dialog-closed':
      return state.status === 'idle' ? { ...state, dialogOpen: false } : state
    case 'started':
      return { status: 'exporting', progress: 0 }
    case 'progressed':
      return state.status === 'exporting'
        ? { status: 'exporting', progress: Math.min(1, Math.max(0, action.progress)) }
        : state
    case 'completed':
      return { status: 'complete', result: action.result }
    case 'failed':
      return { status: 'error', error: action.error }
    case 'cancelled':
      return { status: 'idle', dialogOpen: false, format: null, notice: '書き出しを中止しました。' }
    case 'reset':
      return initialExportState
  }
}

function readableError(error: unknown): string {
  if (error instanceof Error && error.message.length > 0) return error.message
  return '合成動画を書き出せませんでした。もう一度お試しください。'
}

function clockLabel(seconds: number): string {
  const minutes = Math.floor(seconds / 60)
  const remainingSeconds = Math.floor(seconds % 60)
  return `${String(minutes).padStart(2, '0')}:${String(remainingSeconds).padStart(2, '0')}`
}

export function CompositeVideoExport({ event }: CompositeVideoExportProps) {
  const [state, dispatch] = useReducer(exportReducer, initialExportState)
  const abortController = useRef<AbortController | null>(null)
  const trigger = useRef<HTMLButtonElement | null>(null)
  const availability = cameraAvailability(event)
  const exportAvailable = compositeExportPotentiallySupported() && availability.available > 0
  const durationSeconds = eventDuration(event)
  const dialogOpen = state.status === 'idle' && state.dialogOpen && state.format !== null

  useEffect(() => () => abortController.current?.abort(), [])

  function close(): void {
    dispatch({ kind: 'dialog-closed' })
    requestAnimationFrame(() => trigger.current?.focus())
  }

  async function open(): Promise<void> {
    const format = await resolveCompositeRecordingFormat(event)
    if (format === null) {
      dispatch({ kind: 'failed', error: 'このブラウザは動画書き出しに対応していません。' })
      return
    }
    dispatch({ kind: 'dialog-opened', format })
  }

  async function start(): Promise<void> {
    const controller = new AbortController()
    abortController.current = controller
    dispatch({ kind: 'started' })
    try {
      const exported = await exportCompositeVideo({
        event,
        signal: controller.signal,
        onProgress: (progress) => dispatch({ kind: 'progressed', progress }),
      })
      if (controller.signal.aborted) return
      dispatch({ kind: 'completed', result: exported })
    } catch (caught) {
      if (controller.signal.aborted) return
      dispatch({ kind: 'failed', error: readableError(caught) })
    } finally {
      if (abortController.current === controller) abortController.current = null
    }
  }

  function cancel(): void {
    abortController.current?.abort()
    dispatch({ kind: 'cancelled' })
  }

  function save(): void {
    if (state.status !== 'complete') return
    const url = URL.createObjectURL(state.result.blob)
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = `TeslaCam-${event.timestamp}-vehicle.${state.result.extension}`
    anchor.click()
    setTimeout(() => URL.revokeObjectURL(url), 0)
  }

  return (
    <div className="composite-export-control" data-status={state.status}>
      {state.status === 'idle' && (
        <button
          aria-label="合成動画を書き出す"
          className="composite-export-trigger"
          disabled={!exportAvailable}
          ref={trigger}
          type="button"
          onClick={() => void open()}
        >
          <Download aria-hidden="true" size={16} strokeWidth={1.75} />
          書き出す
        </button>
      )}
      {state.status === 'exporting' && (
        <div className="composite-export-toolbar-status" aria-live="polite">
          <div>
            <span>書き出し中</span>
            <span>{`${clockLabel(state.progress * durationSeconds)} / ${clockLabel(durationSeconds)}`}</span>
          </div>
          <progress aria-label="書き出しの進行状況" max={1} value={state.progress} />
          <button type="button" onClick={cancel}>中止</button>
        </div>
      )}
      {state.status === 'complete' && (
        <div className="composite-export-toolbar-result" role="status">
          <span>{`書き出し完了 · ${state.result.extension.toUpperCase()} · 1280 × 720`}</span>
          <button type="button" onClick={save}>
            <Download aria-hidden="true" size={16} strokeWidth={1.75} />
            {state.result.extension.toUpperCase()}を保存
          </button>
          <button
            type="button"
            onClick={() => dispatch({ kind: 'reset' })}
          >
            閉じる
          </button>
        </div>
      )}
      {state.status === 'error' && (
        <div className="composite-export-toolbar-error" role="alert">
          <span>{state.error}</span>
          <button
            type="button"
            onClick={() => void open()}
          >
            もう一度
          </button>
        </div>
      )}
      {state.status === 'idle' && state.notice !== null && <span className="composite-export-notice" role="status">{state.notice}</span>}
      {!exportAvailable && state.status === 'idle' && (
        <span className="composite-export-unavailable">
          {availability.available === 0
            ? '6カメラ映像がないため書き出せません'
            : 'このブラウザは書き出しに未対応です'}
        </span>
      )}
      {dialogOpen && state.status === 'idle' && state.format !== null && (
        <CompositeExportDialog
          event={event}
          format={state.format}
          onCancel={close}
          onStart={() => void start()}
        />
      )}
    </div>
  )
}
