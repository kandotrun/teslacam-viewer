import { FolderOpen, MonitorPlay, ShieldCheck } from 'lucide-react'

export function App() {
  return (
    <main className="welcome-shell">
      <section className="welcome-panel" aria-labelledby="welcome-title">
        <div className="eyebrow">
          <MonitorPlay aria-hidden="true" size={16} strokeWidth={1.8} />
          <span>LOCAL VIDEO WORKSPACE</span>
        </div>

        <div className="welcome-copy">
          <h1 id="welcome-title">
            <span>TeslaCamを、</span>
            <span>ひとつのシアターで。</span>
          </h1>
          <p>
            SSD内のセントリー、保存済み、最近の走行を整理し、最大6方向の映像を同じ時刻で確認できます。
          </p>
        </div>

        <button className="primary-action" type="button">
          <FolderOpen aria-hidden="true" size={20} strokeWidth={1.8} />
          TeslaCamフォルダを選ぶ
        </button>

        <div className="trust-statement">
          <ShieldCheck aria-hidden="true" size={22} strokeWidth={1.8} />
          <div>
            <strong>映像はこのMacだけで処理</strong>
            <p>選択した映像やファイル情報は、外部へ送信されません。</p>
          </div>
        </div>

        <p className="browser-guidance">
          MacのChromeまたはEdgeで、RecentClips、SavedClips、SentryClipsを含むTeslaCamフォルダを選択してください。
        </p>
      </section>
    </main>
  )
}
