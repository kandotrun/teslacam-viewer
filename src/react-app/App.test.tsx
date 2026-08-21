import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import { App } from './App'
import { ScanProgress } from './components/ScanProgress'

describe('welcome view', () => {
  it('shows the folder selection action', () => {
    render(<App />)

    expect(
      screen.getByRole('button', { name: 'TeslaCamフォルダを選ぶ' }),
    ).toBeVisible()
  })

  it('explains that videos stay on this computer', () => {
    render(<App />)

    expect(screen.getByText(/外部へ送信されません/)).toBeVisible()
  })

  it('guides users to a supported desktop browser', () => {
    render(<App />)

    expect(screen.getByText(/ChromeまたはEdge/)).toBeVisible()
  })

  it('announces local scan progress through a live region', () => {
    render(<ScanProgress progress={{ processed: 250, total: 500 }} />)

    expect(screen.getByText('250 / 500 ファイルを確認中です。')).toBeVisible()
    expect(screen.getByText('250 / 500 ファイルを確認中です。').closest('[aria-live]')).toHaveAttribute(
      'aria-live',
      'polite',
    )
  })

  it('announces an indeterminate directory count without a misleading total', () => {
    render(<ScanProgress progress={{ processed: 250, total: 250 }} />)

    expect(screen.getByText('250 ファイルを確認中です。')).toBeVisible()
  })

  it('activates the directory-input fallback when the native picker is unsupported', async () => {
    const user = userEvent.setup()
    const click = vi.spyOn(HTMLInputElement.prototype, 'click')
    Object.defineProperty(window, 'showDirectoryPicker', {
      configurable: true,
      value: undefined,
    })

    render(<App />)
    await user.click(screen.getByRole('button', { name: 'TeslaCamフォルダを選ぶ' }))

    expect(click).toHaveBeenCalled()
  })

  it('clears the fallback input before a folder is selected again', () => {
    render(<App />)
    const fallbackInput = screen.getByLabelText(
      'TeslaCamフォルダを選ぶ（互換モード）',
    )
    Object.defineProperty(fallbackInput, 'value', {
      configurable: true,
      value: 'previous-folder',
      writable: true,
    })

    fireEvent.click(fallbackInput)

    expect(fallbackInput).toHaveValue('')
  })

  it('shows encrypted-file notice after a local library scan and can recover from invalid input', async () => {
    const user = userEvent.setup()
    Object.defineProperty(window, 'showDirectoryPicker', {
      configurable: true,
      value: async () => ({
        name: 'Private Drive',
        values: async function* () {
          yield {
            kind: 'directory',
            name: 'RecentClips',
            values: async function* () {
              yield {
                kind: 'file',
                name: '2026-08-20_20-45-00-front.mp4',
                getFile: async () => new File(['clip'], '2026-08-20_20-45-00-front.mp4', { type: 'video/mp4' }),
              }
            },
          }
          yield {
            kind: 'directory',
            name: 'EncryptedClips',
            values: async function* () {
              yield {
                kind: 'file',
                name: 'hidden.mp4',
                getFile: async () => new File(['clip'], 'hidden.mp4', { type: 'video/mp4' }),
              }
            },
          }
        },
      }),
    })

    render(<App />)
    await user.click(screen.getByRole('button', { name: 'TeslaCamフォルダを選ぶ' }))

    await waitFor(() => {
      expect(screen.getByText('TeslaCam')).toBeVisible()
    })

    expect(screen.queryByText('Private Drive')).not.toBeInTheDocument()
    expect(screen.getByText(/暗号化されたファイル 1 件/)).toBeVisible()
    expect(screen.getByRole('button', { name: 'すべて1' })).toBeVisible()

    const fallbackInput = screen.getByLabelText(
      'TeslaCamフォルダを選ぶ（互換モード）',
    )
    const invalidFile = new File(['clip'], 'clip.mp4')
    Object.defineProperty(invalidFile, 'webkitRelativePath', { value: '' })
    fireEvent.change(fallbackInput, { target: { files: [invalidFile] } })

    await waitFor(() => {
      expect(screen.getByText('TeslaCamフォルダを確認できませんでした。')).toBeVisible()
    })
    await user.click(screen.getByRole('button', { name: 'もう一度選ぶ' }))

    expect(screen.getByRole('button', { name: '別のフォルダを選ぶ' })).toBeVisible()
  })
})
