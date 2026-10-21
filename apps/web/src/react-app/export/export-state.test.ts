import {
  exportFileName,
  exportFormatValue,
  exportProgressClock,
  exportProgressLabel,
  exportReducer,
  exportSaveLabel,
  exportUnavailableReason,
  initialExportState,
  readableExportError,
} from './export-state'

const mp4 = { mimeType: 'video/mp4', extension: 'mp4', label: 'MP4' } as const

describe('exportReducer', () => {
  it('walks from the dialog through progress to a saveable result', () => {
    const opened = exportReducer(initialExportState, { kind: 'dialog-opened', format: mp4 })
    const started = exportReducer(opened, { kind: 'started' })
    const progressed = exportReducer(started, { kind: 'progressed', progress: 1.4 })
    const result = { blob: new Blob(['x']), mimeType: 'video/mp4', extension: 'mp4' } as const

    expect(opened).toEqual({ status: 'idle', dialogOpen: true, format: mp4, notice: null })
    expect(progressed).toEqual({ status: 'exporting', progress: 1 })
    expect(exportReducer(progressed, { kind: 'completed', result })).toEqual({ status: 'complete', result })
  })

  it('returns to the idle control with a notice after an abort', () => {
    const exporting = exportReducer(initialExportState, { kind: 'started' })

    expect(exportReducer(exporting, { kind: 'cancelled' })).toEqual({
      status: 'idle',
      dialogOpen: false,
      format: null,
      notice: '書き出しを中止しました。',
    })
  })

  it('ignores progress outside an export', () => {
    expect(exportReducer(initialExportState, { kind: 'progressed', progress: 0.5 })).toBe(initialExportState)
  })
})

describe('export copy', () => {
  it('explains why export is unavailable', () => {
    expect(exportUnavailableReason(0, true)).toBe('6カメラ映像がないため書き出せません')
    expect(exportUnavailableReason(3, false)).toBe('このブラウザは書き出しに未対応です')
    expect(exportUnavailableReason(6, true)).toBeNull()
  })

  it('names formats, files and the progress button', () => {
    expect(exportFormatValue(mp4)).toBe('MP4')
    expect(exportFormatValue({ ...mp4, fallbackLabel: 'WebM' })).toBe('MP4（失敗時はWebM）')
    expect(exportSaveLabel('webm')).toBe('WEBMを保存')
    expect(exportFileName('2026-08-20_20-45-00', 'mp4')).toBe('TeslaCam-2026-08-20_20-45-00-vehicle.mp4')
    expect(exportProgressLabel(0.426)).toBe('書き出し中 42%')
    expect(exportProgressClock(0.5, 60)).toBe('00:30 / 01:00')
  })

  it('falls back to a generic error message', () => {
    expect(readableExportError(new Error('再生可能な動画がありません。'))).toBe('再生可能な動画がありません。')
    expect(readableExportError('boom')).toBe('合成動画を書き出せませんでした。もう一度お試しください。')
  })
})
