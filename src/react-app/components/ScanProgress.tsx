import { LoaderCircle } from 'lucide-react'

import type { ScanProgress as ScanProgressValue } from '../../teslacam/types'

type ScanProgressProps = {
  readonly progress: ScanProgressValue
}

export function ScanProgress({ progress }: ScanProgressProps) {
  const details =
    progress.processed === 0
      ? 'フォルダを読み込んでいます。'
      : progress.processed === progress.total
        ? `${progress.processed.toLocaleString('ja-JP')} ファイルを確認中です。`
      : `${progress.processed.toLocaleString('ja-JP')} / ${progress.total.toLocaleString('ja-JP')} ファイルを確認中です。`

  return (
    <main className="scan-shell">
      <section className="scan-panel" aria-live="polite" aria-atomic="true">
        <LoaderCircle aria-hidden="true" className="scan-panel__icon" size={28} strokeWidth={1.8} />
        <div>
          <p className="eyebrow">LOCAL SCAN</p>
          <h1>TeslaCamを整理しています</h1>
          <p>{details}</p>
        </div>
      </section>
    </main>
  )
}
