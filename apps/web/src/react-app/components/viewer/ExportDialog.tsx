import { X } from 'lucide-react'
import { useEffect, useRef } from 'react'

import type { TeslaCamEvent } from '../../../teslacam/types'
import { compositeLayoutContract } from '../../design/contract'
import { categoryLabel, eventTime } from '../../event-filters'
import { eventDurationLabel } from '../../event-summary'
import { buildCompositeLayout } from '../../export/composite-layout'
import type { CompositeRecordingFormat } from '../../export/composite-video'
import { exportFormatValue } from '../../export/export-state'

type ExportDialogProps = {
  readonly event: TeslaCamEvent
  readonly format: CompositeRecordingFormat
  readonly onCancel: () => void
  readonly onStart: () => void
}

/** A miniature of the 1280 × 720 export frame, drawn from composite-layout.json. */
function ExportFramePreview() {
  const layout = buildCompositeLayout()
  return (
    <svg aria-hidden="true" className="export-preview" viewBox={`0 0 ${layout.width} ${layout.height}`}>
      <rect className="export-preview__floor" height={layout.height} width={layout.width} />
      {layout.cameras.map(({ cameraId, frame }) => (
        <rect
          className="export-preview__slot"
          height={frame.height * 0.86}
          key={cameraId}
          rx={compositeLayoutContract.videoCornerRadius}
          width={frame.width}
          x={frame.x}
          y={frame.y + frame.height * 0.07}
        />
      ))}
      <image height={layout.vehicle.height} href="/vehicle-top.png" width={layout.vehicle.width} x={layout.vehicle.x} y={layout.vehicle.y} />
    </svg>
  )
}

export function ExportDialog({ event, format, onCancel, onStart }: ExportDialogProps) {
  const dialog = useRef<HTMLDialogElement>(null)

  useEffect(() => {
    const element = dialog.current
    if (element === null) return
    if (typeof element.showModal === 'function') element.showModal()
    else element.setAttribute('open', '')
  }, [])

  return (
    <dialog
      aria-describedby="export-dialog-description"
      aria-labelledby="export-dialog-title"
      className="export-dialog"
      ref={dialog}
      onCancel={(cancel) => {
        cancel.preventDefault()
        onCancel()
      }}
    >
      <div className="export-dialog__head">
        <h2 id="export-dialog-title">合成動画を書き出す</h2>
        <button aria-label="書き出し画面を閉じる" className="icon-button" type="button" onClick={onCancel}>
          <X aria-hidden="true" size={20} />
        </button>
      </div>
      <p className="export-dialog__text" id="export-dialog-description">
        選択中の記録を、6カメラと車両配置を含む<span className="nowrap">1本の動画にまとめます。</span><span className="nowrap">利用できないカメラは「記録なし」と表示します。</span>
      </p>
      <ExportFramePreview />
      <dl className="export-dialog__facts">
        <div>
          <dt>記録</dt>
          <dd><span className="num">{eventTime(event.timestamp)}</span> {categoryLabel(event.category)}</dd>
        </div>
        <div>
          <dt>範囲</dt>
          <dd>イベント全体・{eventDurationLabel(event)}</dd>
        </div>
        <div>
          <dt>解像度</dt>
          <dd className="num">1280 × 720</dd>
        </div>
        <div>
          <dt>形式</dt>
          <dd>{exportFormatValue(format)}</dd>
        </div>
      </dl>
      <p className="export-dialog__notice">
        <span className="nowrap">対応ブラウザでは高速に書き出します。</span><span className="nowrap">完了まではタブを閉じないでください。</span>
      </p>
      <div className="export-dialog__actions">
        <button className="text-button" type="button" onClick={onCancel}>キャンセル</button>
        <button className="primary-button" type="button" onClick={onStart}>書き出しを開始</button>
      </div>
    </dialog>
  )
}
