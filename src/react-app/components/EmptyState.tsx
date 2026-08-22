import { FolderOpen, MonitorPlay, ShieldCheck } from 'lucide-react'

type EmptyStateProps = {
  readonly error: 'denied' | 'invalid' | 'failed' | null
  readonly onSelect: () => void
}

function errorCopy(error: EmptyStateProps['error']): {
  readonly title: string
  readonly description: string
  readonly action: string
} {
  switch (error) {
    case 'denied':
      return {
        title: 'フォルダへのアクセスが許可されませんでした。',
        description: 'TeslaCamフォルダへの読み取りアクセスを許可して、もう一度選んでください。',
        action: 'もう一度選ぶ',
      }
    case 'invalid':
      return {
        title: 'TeslaCamフォルダを確認できませんでした。',
        description: 'RecentClips、SavedClips、SentryClipsを含むフォルダを選んでください。',
        action: 'もう一度選ぶ',
      }
    case 'failed':
      return {
        title: 'フォルダの読み取りを完了できませんでした。',
        description: 'SSDの接続を確認してから、もう一度選んでください。',
        action: 'もう一度選ぶ',
      }
    case null:
      return {
        title: 'TeslaCamを、ひとつのシアターで。',
        description: 'SSD内のセントリー、保存済み、最近の走行を整理し、最大6方向の映像を同じ時刻で確認できます。',
        action: 'TeslaCamフォルダを選ぶ',
      }
  }
}

export function EmptyState({ error, onSelect }: EmptyStateProps) {
  const copy = errorCopy(error)

  return (
    <main className="welcome-shell">
      <section className="welcome-panel" aria-labelledby="welcome-title">
        <div className="eyebrow">
          <MonitorPlay aria-hidden="true" size={16} strokeWidth={1.8} />
          <span>LOCAL VIDEO WORKSPACE</span>
        </div>
        <div className="welcome-copy">
          <h1 aria-label={copy.title} id="welcome-title">
            {error === null ? (
              <>
                <span>TeslaCamを、</span>
                <span className="welcome-title__phrase">ひとつのシアターで。</span>
              </>
            ) : copy.title}
          </h1>
          <p>
            {error === null ? (
              <>
                SSD内のセントリー、<span className="text-keep">保存済み</span>、<span className="text-keep">最近の走行</span>を整理し、<span className="text-keep">最大6方向</span>の映像を<span className="text-keep">同じ時刻</span>で確認できます。
              </>
            ) : copy.description}
          </p>
        </div>
        <button className="primary-action" type="button" onClick={onSelect}>
          <FolderOpen aria-hidden="true" size={20} strokeWidth={1.8} />
          {copy.action}
        </button>
        <div className="trust-statement">
          <ShieldCheck aria-hidden="true" size={22} strokeWidth={1.8} />
          <div>
            <strong>映像はこのMacだけで処理</strong>
            <p>選択した映像やファイル情報は、<span className="text-keep">外部へ送信されません。</span></p>
          </div>
        </div>
        <p className="browser-guidance">
          MacのChromeまたはEdgeで、RecentClips、SavedClips、SentryClipsを含む<span className="text-keep">TeslaCamフォルダ</span>を選択してください。
        </p>
      </section>
    </main>
  )
}
