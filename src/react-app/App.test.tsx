import { render, screen } from '@testing-library/react'

import { App } from './App'
import { ScanProgress } from './components/ScanProgress'

describe('welcome view', () => {
  it('shows the folder selection action', () => {
    render(<App />)
    expect(screen.getByRole('button', { name: 'TeslaCamフォルダを選ぶ' })).toBeVisible()
  })

  it('keeps the welcome view focused on folder selection', () => {
    render(<App />)
    expect(screen.queryByRole('note')).not.toBeInTheDocument()
  })

  it('guides users to a supported desktop browser', () => {
    render(<App />)
    expect(screen.getByText(/ChromeまたはEdge/)).toBeVisible()
  })

  it('announces local scan progress through a live region', () => {
    render(<ScanProgress progress={{ processed: 250, total: 500 }} />)
    const progress = screen.getByText('250 / 500 ファイルを確認中です。')
    expect(progress).toBeVisible()
    expect(progress.closest('[aria-live]')).toHaveAttribute('aria-live', 'polite')
  })

  it('announces an indeterminate directory count without a misleading total', () => {
    render(<ScanProgress progress={{ processed: 250, total: 250 }} />)
    expect(screen.getByText('250 ファイルを確認中です。')).toBeVisible()
  })
})
