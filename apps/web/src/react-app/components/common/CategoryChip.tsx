import { Bookmark, Route, Shield } from 'lucide-react'

import type { ClipCategory } from '../../../teslacam/types'
import { categoryLabel } from '../../event-filters'
import { shortCategoryLabels } from '../../presentation/event-copy'

export function CategoryIcon({ category, size }: { readonly category: ClipCategory; readonly size: number }) {
  switch (category) {
    case 'sentry':
      return <Shield aria-hidden="true" size={size} strokeWidth={2.2} />
    case 'saved':
      return <Bookmark aria-hidden="true" size={size} strokeWidth={2.2} />
    case 'recent':
      return <Route aria-hidden="true" size={size} strokeWidth={2.2} />
  }
}

type CategoryChipProps = {
  readonly category: ClipCategory
  /** `full` shows `最近の走行`, `short` shows `走行`. */
  readonly wording: 'full' | 'short'
  readonly size?: 'regular' | 'large'
}

/** Category word + icon + colour: never colour alone (DESIGN.md §1.4). */
export function CategoryChip({ category, wording, size = 'regular' }: CategoryChipProps) {
  return (
    <span className="chip" data-category={category} data-size={size}>
      <CategoryIcon category={category} size={size === 'large' ? 14 : 13} />
      {wording === 'full' ? categoryLabel(category) : shortCategoryLabels[category]}
    </span>
  )
}
