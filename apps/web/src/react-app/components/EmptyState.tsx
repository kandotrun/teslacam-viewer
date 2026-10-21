import { FolderOpen } from 'lucide-react'

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
        title: 'SSDのTeslaCamフォルダを開く',
        description: 'セントリー、保存済み、最近の走行を日時順に整理して、最大6方向の映像を確認します。',
        action: 'TeslaCamフォルダを選ぶ',
      }
  }
}

export function EmptyState({ error, onSelect }: EmptyStateProps) {
  const copy = errorCopy(error)

  return (
    <main className="welcome-shell">
      <section className="welcome-panel" aria-labelledby="welcome-title">
        <header className="welcome-brand">
          <strong>TeslaCam Viewer</strong>
        </header>
        <div className="welcome-copy">
          <h1 id="welcome-title">
            {error === null ? <>SSDのTeslaCam<span className="welcome-title__tail">フォルダを開く</span></> : copy.title}
          </h1>
          <p>{copy.description}</p>
        </div>
        <button className="primary-action" type="button" onClick={onSelect}>
          <FolderOpen aria-hidden="true" size={20} strokeWidth={1.8} />
          {copy.action}
        </button>
        {error === null && (
          <dl className="welcome-outcome">
            <div>
              <dt>選ぶもの</dt>
              <dd>RecentClips、SavedClips、SentryClipsを含むTeslaCamフォルダ</dd>
            </div>
            <div>
              <dt>表示されるもの</dt>
              <dd>日時順のイベント一覧と、最大6方向の同期映像</dd>
            </div>
          </dl>
        )}
        <p className="browser-guidance">
          MacのChromeまたはEdgeに対応しています。
        </p>
      </section>
    </main>
  )
}
