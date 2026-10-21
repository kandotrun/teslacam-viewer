import { FolderOpen } from 'lucide-react'

type AppHeaderProps = {
  readonly onRescan: () => void
}

export function AppHeader({ onRescan }: AppHeaderProps) {
  return (
    <header className="app-header">
      <div className="app-header__identity">
        <strong>TeslaCam Viewer</strong>
      </div>
      <div className="app-header__actions">
        <button className="quiet-action" type="button" onClick={onRescan}>
          <FolderOpen aria-hidden="true" size={18} strokeWidth={1.8} />
          別のフォルダを選ぶ
        </button>
      </div>
    </header>
  )
}
