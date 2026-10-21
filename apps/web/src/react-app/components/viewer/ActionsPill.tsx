import { Car, CircleAlert, CircleCheck, Download, Film, LayoutGrid, X } from 'lucide-react'
import { useRef, useState } from 'react'

import type { TeslaCamEvent } from '../../../teslacam/types'
import { cameraAvailability } from '../../event-summary'
import { compositeExportPotentiallySupported } from '../../export/composite-video'
import {
  exportProgressClock,
  exportProgressPercent,
  exportSaveLabel,
  exportUnavailableReason,
  type ExportState,
} from '../../export/export-state'
import { eventDuration } from '../../playback/timeline'
import type { ViewMode } from '../../presentation/stage-layout'
import { SegmentedControl, type SegmentedOption } from '../common/SegmentedControl'
import { ExportDialog } from './ExportDialog'
import type { CompositeExportControl } from './use-composite-export'

type ActionsPillProps = {
  readonly event: TeslaCamEvent
  readonly modes: readonly ViewMode[]
  readonly mode: ViewMode
  /** Every camera failed and `event.mp4` takes over; the other views are unavailable. */
  readonly representativeForced: boolean
  readonly onMode: (mode: ViewMode) => void
  readonly exporter: CompositeExportControl
}

const viewOptions: Readonly<Record<ViewMode, Omit<SegmentedOption<ViewMode>, 'value' | 'disabled'>>> = {
  spatial: { name: '車両配置', label: '車両配置', icon: <Car aria-hidden="true" size={18} /> },
  grid: { name: '6分割', label: '6分割', icon: <LayoutGrid aria-hidden="true" size={18} /> },
  representative: { name: '代表映像', label: '代表映像', icon: <Film aria-hidden="true" size={18} /> },
}

function ProgressRing({ progress }: { readonly progress: number }) {
  const circumference = 2 * Math.PI * 8
  return (
    <svg aria-hidden="true" className="progress-ring" height="20" viewBox="0 0 20 20" width="20">
      <circle cx="10" cy="10" r="8" />
      <circle cx="10" cy="10" r="8" strokeDasharray={circumference} strokeDashoffset={circumference * (1 - progress)} />
    </svg>
  )
}

function ExportControl({ event, exporter }: { readonly event: TeslaCamEvent; readonly exporter: CompositeExportControl }) {
  const trigger = useRef<HTMLButtonElement>(null)
  const { state } = exporter
  // A popover belongs to the status it was opened in and closes when the status changes.
  const [popoverStatus, setPopoverStatus] = useState<ExportState['status'] | null>(null)
  const popoverOpen = popoverStatus === state.status
  const togglePopover = (): void => setPopoverStatus(popoverOpen ? null : state.status)
  const reason = exportUnavailableReason(cameraAvailability(event).available, compositeExportPotentiallySupported())
  const dialogOpen = state.status === 'idle' && state.dialogOpen && state.format !== null
  const durationSeconds = eventDuration(event)

  function closeDialog(): void {
    exporter.closeDialog()
    requestAnimationFrame(() => trigger.current?.focus())
  }

  return (
    <div className="export-control" data-status={state.status}>
      {state.status === 'idle' && (
        <button
          aria-describedby={reason === null ? undefined : 'export-unavailable-reason'}
          aria-disabled={reason !== null}
          aria-label="合成動画を書き出す"
          className="action-button"
          ref={trigger}
          type="button"
          onClick={() => {
            if (reason === null) void exporter.open()
            else togglePopover()
          }}
        >
          <Download aria-hidden="true" size={18} />
          <span className="action-button__text">書き出す</span>
        </button>
      )}
      {state.status === 'idle' && reason !== null && (
        <p className="popover export-popover" data-open={popoverOpen} id="export-unavailable-reason">{reason}</p>
      )}
      {state.status === 'idle' && state.notice !== null && (
        <p className="popover export-popover" data-open="true" role="status">{state.notice}</p>
      )}
      {state.status === 'exporting' && (
        <>
          <button
            aria-expanded={popoverOpen}
            className="action-button action-button--progress"
            type="button"
            onClick={togglePopover}
          >
            <ProgressRing progress={state.progress} />
            <span className="action-button__text">
              <span>書き出し中</span> <span className="num">{exportProgressPercent(state.progress)}</span>
            </span>
          </button>
          <div aria-live="polite" className="popover export-popover" data-open={popoverOpen}>
            <p className="num">{exportProgressClock(state.progress, durationSeconds)}</p>
            <progress aria-label="書き出しの進行状況" max={1} value={state.progress} />
            <button className="text-button" type="button" onClick={exporter.cancel}>中止</button>
          </div>
        </>
      )}
      {state.status === 'complete' && (
        <>
          <span className="export-control__done" role="status">
            <CircleCheck aria-hidden="true" size={18} />
            <span className="action-button__text">書き出し完了</span>
          </span>
          <button className="primary-button primary-button--small" type="button" onClick={exporter.save}>
            <Download aria-hidden="true" size={16} />
            {exportSaveLabel(state.result.extension)}
          </button>
          <button aria-label="閉じる" className="icon-button" type="button" onClick={exporter.reset}>
            <X aria-hidden="true" size={18} />
          </button>
        </>
      )}
      {state.status === 'error' && (
        <>
          <button className="action-button" type="button" onClick={() => void exporter.open()}>
            <CircleAlert aria-hidden="true" size={18} />
            <span>もう一度</span>
          </button>
          <p className="popover export-popover" data-open="true" role="alert">{state.error}</p>
        </>
      )}
      {dialogOpen && state.format !== null && (
        <ExportDialog event={event} format={state.format} onCancel={closeDialog} onStart={() => void exporter.start()} />
      )}
    </div>
  )
}

/** View switch and export, floating at the top right (DESIGN.md §6 Actions pill). */
export function ActionsPill({ event, modes, mode, representativeForced, onMode, exporter }: ActionsPillProps) {
  return (
    <div className="pill actions-pill">
      {modes.length > 1 && (
        <SegmentedControl
          className="segmented segmented--pill"
          label="表示"
          options={modes.map((value) => ({
            value,
            ...viewOptions[value],
            disabled: representativeForced && value !== 'representative',
          }))}
          value={mode}
          onChange={onMode}
        />
      )}
      <ExportControl event={event} exporter={exporter} />
    </div>
  )
}
