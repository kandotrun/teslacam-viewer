/**
 * State of the export control (parity spec §2.11) and its copy. The control keeps one name through the
 * flow: 書き出す → 書き出し中 → 書き出し完了 → MP4を保存 (DESIGN.md §8).
 */
import type { CompositeRecordingFormat, CompositeVideoExportResult } from './composite-video'

export type ExportState =
  | { readonly status: 'idle'; readonly dialogOpen: boolean; readonly format: CompositeRecordingFormat | null; readonly notice: string | null }
  | { readonly status: 'exporting'; readonly progress: number }
  | { readonly status: 'complete'; readonly result: CompositeVideoExportResult }
  | { readonly status: 'error'; readonly error: string }

export type ExportAction =
  | { readonly kind: 'dialog-opened'; readonly format: CompositeRecordingFormat }
  | { readonly kind: 'dialog-closed' }
  | { readonly kind: 'started' }
  | { readonly kind: 'progressed'; readonly progress: number }
  | { readonly kind: 'completed'; readonly result: CompositeVideoExportResult }
  | { readonly kind: 'failed'; readonly error: string }
  | { readonly kind: 'cancelled' }
  | { readonly kind: 'reset' }

export const initialExportState: ExportState = { status: 'idle', dialogOpen: false, format: null, notice: null }

export const EXPORT_ABORTED_NOTICE = '書き出しを中止しました。'
export const EXPORT_GENERIC_ERROR = '合成動画を書き出せませんでした。もう一度お試しください。'
export const EXPORT_UNSUPPORTED_ERROR = 'このブラウザは動画書き出しに対応していません。'

export function exportReducer(state: ExportState, action: ExportAction): ExportState {
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
      return { status: 'idle', dialogOpen: false, format: null, notice: EXPORT_ABORTED_NOTICE }
    case 'reset':
      return initialExportState
  }
}

/** Why export is not offered, or null when it is. */
export function exportUnavailableReason(available: number, encoderSupported: boolean): string | null {
  if (available === 0) return '6カメラ映像がないため書き出せません'
  if (!encoderSupported) return 'このブラウザは書き出しに未対応です'
  return null
}

/** `MP4` or `MP4（失敗時はWebM）`. */
export function exportFormatValue(format: CompositeRecordingFormat): string {
  return format.fallbackLabel === undefined ? format.label : `${format.label}（失敗時は${format.fallbackLabel}）`
}

export function exportSaveLabel(extension: 'mp4' | 'webm'): string {
  return `${extension.toUpperCase()}を保存`
}

export function exportFileName(timestamp: string, extension: 'mp4' | 'webm'): string {
  return `TeslaCam-${timestamp}-vehicle.${extension}`
}

/** `書き出し中 42%` */
export function exportProgressLabel(progress: number): string {
  return `書き出し中 ${Math.floor(Math.min(1, Math.max(0, progress)) * 100)}%`
}

/** `00:30 / 01:00` over the nominal event duration. */
export function exportProgressClock(progress: number, durationSeconds: number): string {
  return `${exportClockLabel(progress * durationSeconds)} / ${exportClockLabel(durationSeconds)}`
}

export function exportClockLabel(seconds: number): string {
  const minutes = Math.floor(seconds / 60)
  const remainder = Math.floor(seconds % 60)
  return `${String(minutes).padStart(2, '0')}:${String(remainder).padStart(2, '0')}`
}

export function readableExportError(error: unknown): string {
  return error instanceof Error && error.message.length > 0 ? error.message : EXPORT_GENERIC_ERROR
}
