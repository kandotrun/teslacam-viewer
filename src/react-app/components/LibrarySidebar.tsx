import { CalendarDays, Search, ShieldAlert } from 'lucide-react'

import type { ClipCategory, ScanWarning } from '../../teslacam/types'
import type { EventFilters } from '../app-state'

type CategoryCounts = Readonly<Record<ClipCategory, number>>

type LibrarySidebarProps = {
  readonly counts: CategoryCounts
  readonly filters: EventFilters
  readonly warnings: readonly ScanWarning[]
  readonly onCategory: (category: EventFilters['category']) => void
  readonly onDate: (dateKey: string | null) => void
  readonly onQuery: (query: string) => void
}

const categories = [
  { key: 'all', label: 'すべて' },
  { key: 'recent', label: '最近の走行' },
  { key: 'saved', label: '保存済み' },
  { key: 'sentry', label: 'セントリー' },
] as const satisfies readonly {
  readonly key: EventFilters['category']
  readonly label: string
}[]

function warningCopy(warning: ScanWarning): string {
  switch (warning.kind) {
    case 'encrypted-file':
      return `暗号化されたファイル ${warning.count} 件は読み込みませんでした。`
    case 'empty-media-file':
      return `空の映像ファイル ${warning.count} 件を除外しました。`
    case 'invalid-metadata-file':
      return `読み取れないイベント情報 ${warning.count} 件を除外しました。`
    case 'unknown-file':
      return `対応外のファイル ${warning.count} 件を除外しました。`
  }
}

export function LibrarySidebar({
  counts,
  filters,
  warnings,
  onCategory,
  onDate,
  onQuery,
}: LibrarySidebarProps) {
  const allCount = counts.recent + counts.saved + counts.sentry

  return (
    <aside className="library-sidebar" aria-label="ライブラリの絞り込み">
      <nav aria-label="映像カテゴリ">
        <p className="sidebar-label">LIBRARY</p>
        <div className="category-list">
          {categories.map((category) => {
            const count = category.key === 'all' ? allCount : counts[category.key]
            const isActive = filters.category === category.key
            return (
              <button
                aria-current={isActive ? 'page' : undefined}
                className="category-action"
                data-active={isActive}
                key={category.key}
                type="button"
                onClick={() => onCategory(category.key)}
              >
                <span>{category.label}</span>
                <span>{count}</span>
              </button>
            )
          })}
        </div>
      </nav>
      <div className="sidebar-filters">
        <label className="filter-field">
          <span>
            <CalendarDays aria-hidden="true" size={16} strokeWidth={1.8} />
            日付
          </span>
          <input
            type="date"
            value={filters.dateKey ?? ''}
            onChange={(event) => onDate(event.currentTarget.value || null)}
          />
        </label>
        <label className="filter-field">
          <span>
            <Search aria-hidden="true" size={16} strokeWidth={1.8} />
            検索
          </span>
          <input
            placeholder="日時またはカテゴリ"
            type="search"
            value={filters.query}
            onChange={(event) => onQuery(event.currentTarget.value)}
          />
        </label>
      </div>
      {warnings.length > 0 ? (
        <section className="scan-warnings" aria-label="読み込み時の注意">
          <ShieldAlert aria-hidden="true" size={18} strokeWidth={1.8} />
          <div>
            <p className="sidebar-label">SCAN NOTICE</p>
            {warnings.map((warning) => (
              <p key={warning.kind}>{warningCopy(warning)}</p>
            ))}
          </div>
        </section>
      ) : null}
    </aside>
  )
}
