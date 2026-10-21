import { render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach } from 'vitest'

import { App } from './App'
import { WelcomeContent } from './components/library/WelcomeContent'

afterEach(() => {
  Reflect.deleteProperty(window, 'showDirectoryPicker')
})

describe('welcome', () => {
  it('invites the user to choose the TeslaCam folder from the library panel', () => {
    render(<App />)
    const library = screen.getByRole('complementary', { name: 'ライブラリ' })

    expect(within(library).getByRole('heading', { name: 'TeslaCamの記録を見る' })).toBeVisible()
    expect(within(library).getByRole('button', { name: 'フォルダを選ぶ' })).toBeVisible()
    expect(within(library).getByText('MacのChromeまたはEdgeに対応しています。')).toBeVisible()
    expect(within(library).getByText('セントリーは検知の15秒前から')).toBeVisible()
    expect(within(library).getByText('6カメラを1本の動画に')).toBeVisible()
  })

  it('shows the car with six empty camera windows on the stage', () => {
    render(<App />)
    const stage = screen.getByRole('main', { name: '6台のカメラ' })

    expect(stage).toHaveAttribute('data-mode', 'empty')
    expect([...stage.querySelectorAll('.camera-window')].map((element) => element.textContent)).toEqual([
      'フロント',
      'リア',
      '左ピラー',
      '左リピーター',
      '右ピラー',
      '右リピーター',
    ])
  })
})

describe('scanning and errors', () => {
  it('counts scanned entries in a polite live region and can be cancelled', async () => {
    const user = userEvent.setup()
    let cancelled = false
    render(<WelcomeContent compatibilityMode={false} error={null} progress={{ processed: 1250, total: 1250 }} onCancelScan={() => { cancelled = true }} onSelect={() => undefined} />)

    const count = screen.getByText('1,250')
    expect(count.closest('[aria-live]')).toHaveAttribute('aria-live', 'polite')
    expect(count.closest('p')).toHaveTextContent('1,250件を確認しました')
    expect(screen.getByRole('progressbar', { name: '読み込みの進行状況' })).toBeInTheDocument()
    await user.click(screen.getByRole('button', { name: '中止' }))
    expect(cancelled).toBe(true)
  })

  it('replaces the paragraph with what happened and what to do', () => {
    render(<WelcomeContent compatibilityMode={false} error="invalid" progress={null} onCancelScan={() => undefined} onSelect={() => undefined} />)

    expect(screen.getByRole('alert')).toHaveTextContent('TeslaCamフォルダを確認できませんでした。RecentClips、SavedClips、SentryClipsを含むフォルダを選んでください。')
    expect(screen.getByRole('button', { name: 'もう一度選ぶ' })).toBeVisible()
    expect(screen.queryByText(/ごめんなさい|申し訳/u)).not.toBeInTheDocument()
  })
})
