import { FolderOpen, ShieldCheck } from 'lucide-react'

type AppHeaderProps = {
  readonly folderName: string
  readonly onRescan: () => void
}

export function AppHeader({ folderName, onRescan }: AppHeaderProps) {
  return (
    <header className="app-header">
      <div className="app-header__identity">
        <strong>TeslaCam Viewer</strong>
        <span className="app-header__source">{folderName}</span>
      </div>
      <div className="app-header__actions">
        <span className="app-header__privacy">
          <ShieldCheck aria-hidden="true" size={16} strokeWidth={1.8} />
          このMacだけで処理
        </span>
        <button className="quiet-action" type="button" onClick={onRescan}>
          <FolderOpen aria-hidden="true" size={18} strokeWidth={1.8} />
          別のフォルダを選ぶ
        </button>
      </div>
    </header>
  )
}
