import { Download, Folder, Shield } from 'lucide-react'

import type { ScanProgress } from '../../../teslacam/types'
import type { AppState } from '../../app-state'
import { scanProgressCopy } from '../../presentation/event-copy'

type ErrorReason = Extract<AppState, { readonly kind: 'error' }>['reason']

type WelcomeContentProps = {
  readonly error: ErrorReason | null
  readonly progress: ScanProgress | null
  readonly compatibilityMode: boolean
  readonly onSelect: () => void
  readonly onCancelScan: () => void
}

const errorCopy: Readonly<Record<ErrorReason, { readonly title: string; readonly description: string }>> = {
  denied: {
    title: 'フォルダへのアクセスが許可されませんでした。',
    description: 'TeslaCamフォルダへの読み取りアクセスを許可して、もう一度選んでください。',
  },
  invalid: {
    title: 'TeslaCamフォルダを確認できませんでした。',
    description: 'RecentClips、SavedClips、SentryClipsを含むフォルダを選んでください。',
  },
  failed: {
    title: 'フォルダの読み取りを完了できませんでした。',
    description: 'SSDの接続を確認してから、もう一度選んでください。',
  },
}

function ScanProgressBlock({ progress, onCancel }: { readonly progress: ScanProgress; readonly onCancel: () => void }) {
  const copy = scanProgressCopy(progress)
  return (
    <div className="scan-progress">
      <div className="scan-progress__head">
        <span className="scan-progress__title">読み込み中</span>
        <button className="text-button" type="button" onClick={onCancel}>中止</button>
      </div>
      <p aria-atomic="true" aria-live="polite" className="scan-progress__count">
        {copy.count === null ? copy.label : <><span className="num">{copy.count}</span>{copy.label}</>}
      </p>
      <div aria-label="読み込みの進行状況" className="scan-progress__bar" role="progressbar" />
    </div>
  )
}

/** Welcome, scanning and error states of the library panel (DESIGN.md §6). */
export function WelcomeContent({ error, progress, compatibilityMode, onSelect, onCancelScan }: WelcomeContentProps) {
  const failure = error === null ? null : errorCopy[error]
  return (
    <section aria-labelledby="welcome-title" className="welcome">
      <h1 className="welcome__title" id="welcome-title">TeslaCamの<wbr />記録を見る</h1>
      {failure === null ? (
        <p className="welcome__lead">
          USBドライブの TeslaCam フォルダを選ぶと、セントリーと保存したクリップ、最近の走行を日ごとに並べて、6台のカメラを同時に再生します。
        </p>
      ) : (
        <div className="welcome__error" role="alert">
          <strong>{failure.title}</strong>
          <p>{failure.description}</p>
        </div>
      )}
      {progress === null ? (
        <button className="cta" type="button" onClick={onSelect}>
          <Folder aria-hidden="true" size={18} />
          {failure === null ? 'フォルダを選ぶ' : 'もう一度選ぶ'}
        </button>
      ) : (
        <ScanProgressBlock progress={progress} onCancel={onCancelScan} />
      )}
      <p className="welcome__browser">MacのChromeまたはEdgeに対応しています。</p>
      {compatibilityMode && (
        <p aria-label="ブラウザ互換性" className="welcome__browser" role="status">
          このブラウザでは互換モードを使用します。フォルダ選択画面からTeslaCamフォルダを選んでください。
        </p>
      )}
      <ul className="welcome__notes">
        <li>
          <Shield aria-hidden="true" size={20} />
          <span><b>セントリーは検知の15秒前から</b>何が近づいたのかをすぐ確かめられます。</span>
        </li>
        <li>
          <Download aria-hidden="true" size={20} />
          <span><b>6カメラを1本の動画に</b>車両の配置どおりに並べて書き出せます。</span>
        </li>
      </ul>
    </section>
  )
}
