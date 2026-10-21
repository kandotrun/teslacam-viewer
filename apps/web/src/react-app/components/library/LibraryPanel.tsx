import { Folder, Search, X } from 'lucide-react'
import type { ReactNode } from 'react'

type LibraryPanelProps = {
  /** Folder name of a loaded library; the header shows search and the folder chip only then. */
  readonly folderName: string | null
  readonly searchOpen?: boolean
  readonly onToggleSearch?: () => void
  readonly onRescan?: () => void
  readonly hidden?: boolean
  readonly children: ReactNode
}

/** The floating library: wordmark, folder chip and the content of the current app state. */
export function LibraryPanel({ folderName, searchOpen = false, onToggleSearch, onRescan, hidden = false, children }: LibraryPanelProps) {
  return (
    <aside aria-label="ライブラリ" className="library" data-hidden={hidden} hidden={hidden}>
      <header className="library__head">
        <span className="wordmark">TeslaCam Viewer</span>
        {folderName !== null && (
          <>
            <button
              aria-expanded={searchOpen}
              aria-label={searchOpen ? '検索を閉じる' : '検索を開く'}
              className="icon-button"
              type="button"
              onClick={onToggleSearch}
            >
              {searchOpen ? <X aria-hidden="true" size={18} /> : <Search aria-hidden="true" size={18} />}
            </button>
            <button aria-label="別のフォルダを選ぶ" className="folder-chip" title="別のフォルダを選ぶ" type="button" onClick={onRescan}>
              <Folder aria-hidden="true" size={16} />
              <span className="folder-chip__name">{folderName}</span>
            </button>
          </>
        )}
      </header>
      {children}
    </aside>
  )
}
