import { Film, ShieldCheck, X } from 'lucide-react'
import { useEffect, useRef } from 'react'

import type { TeslaCamEvent } from '../../teslacam/types'
import type { CompositeRecordingFormat } from '../export/composite-video'
import { categoryLabel, eventTime } from '../event-filters'
import { eventDurationLabel } from '../event-summary'

type CompositeExportDialogProps = {
  readonly event: TeslaCamEvent
  readonly format: CompositeRecordingFormat
  readonly onCancel: () => void
  readonly onStart: () => void
}

export function CompositeExportDialog({
  event,
  format,
  onCancel,
  onStart,
}: CompositeExportDialogProps) {
  const dialog = useRef<HTMLDialogElement | null>(null)

  useEffect(() => {
    if (dialog.current === null) return
    if (typeof dialog.current.showModal === 'function') dialog.current.showModal()
    else dialog.current.setAttribute('open', '')
  }, [])

  return (
    <dialog
      aria-describedby="composite-export-description"
      aria-labelledby="composite-export-title"
      className="composite-export-dialog"
      ref={dialog}
      onCancel={(event) => {
        event.preventDefault()
        onCancel()
      }}
    >
      <header>
        <div>
          <Film aria-hidden="true" size={18} strokeWidth={1.75} />
          <h2 id="composite-export-title">合成動画を書き出す</h2>
        </div>
        <button aria-label="書き出し画面を閉じる" type="button" onClick={onCancel}>
          <X aria-hidden="true" size={18} strokeWidth={1.75} />
        </button>
      </header>
      <div className="composite-export-dialog__body">
        <p id="composite-export-description">
          選択中の記録を、6カメラと車両配置を含む<span className="composite-export-dialog__atomic">1本の動画にまとめます。</span><span className="composite-export-dialog__atomic">利用できないカメラは「記録なし」と表示します。</span>
        </p>
        <dl>
          <div>
            <dt>記録</dt>
            <dd>{eventTime(event.timestamp)} · {categoryLabel(event.category)}</dd>
          </div>
          <div>
            <dt>範囲</dt>
            <dd>イベント全体・{eventDurationLabel(event)}</dd>
          </div>
          <div>
            <dt>解像度</dt>
            <dd>1280 × 720</dd>
          </div>
          <div>
            <dt>形式</dt>
            <dd>{format.label}{format.fallbackLabel === undefined ? '' : `（失敗時は${format.fallbackLabel}）`}</dd>
          </div>
        </dl>
        <p className="composite-export-dialog__privacy">
          <ShieldCheck aria-hidden="true" size={16} strokeWidth={1.75} />
          <span>映像はこのMacの中だけで処理され、<span className="composite-export-dialog__atomic">外部へ送信されません。</span></span>
        </p>
        <p className="composite-export-dialog__notice">
          <span className="composite-export-dialog__atomic">対応ブラウザでは高速に書き出します。</span><span className="composite-export-dialog__atomic">完了まではタブを閉じないでください。</span>
        </p>
      </div>
      <footer>
        <button className="composite-export-secondary" type="button" onClick={onCancel}>キャンセル</button>
        <button className="composite-export-primary" type="button" onClick={onStart}>書き出しを開始</button>
      </footer>
    </dialog>
  )
}
