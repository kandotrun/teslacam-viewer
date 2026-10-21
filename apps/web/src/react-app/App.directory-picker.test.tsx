import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, vi } from 'vitest'

import { App } from './App'

afterEach(() => vi.restoreAllMocks())

describe('directory picker', () => {
  it('activates the directory-input fallback when the native picker is unsupported', async () => {
    const user = userEvent.setup()
    const click = vi.spyOn(HTMLInputElement.prototype, 'click')
    Object.defineProperty(window, 'showDirectoryPicker', { configurable: true, value: undefined })
    render(<App />)
    expect(screen.getByRole('status', { name: 'ブラウザ互換性' })).toHaveTextContent('互換モード')
    await user.click(screen.getByRole('button', { name: 'TeslaCamフォルダを選ぶ' }))
    expect(click).toHaveBeenCalled()
  })

  it('recovers from a denied native picker by using the fallback picker', async () => {
    const user = userEvent.setup()
    const click = vi.spyOn(HTMLInputElement.prototype, 'click')
    Object.defineProperty(window, 'showDirectoryPicker', {
      configurable: true,
      value: async () => Promise.reject(new DOMException('', 'NotAllowedError')),
    })
    render(<App />)
    await user.click(screen.getByRole('button', { name: 'TeslaCamフォルダを選ぶ' }))
    await waitFor(() => expect(screen.getByText('フォルダへのアクセスが許可されませんでした。')).toBeVisible())
    Object.defineProperty(window, 'showDirectoryPicker', { configurable: true, value: undefined })
    await user.click(screen.getByRole('button', { name: 'もう一度選ぶ' }))
    expect(click).toHaveBeenCalled()
  })

  it('maps an unknown picker rejection to a safe scan failure', async () => {
    const user = userEvent.setup()
    Object.defineProperty(window, 'showDirectoryPicker', {
      configurable: true,
      value: async () => Promise.reject('picker unavailable'),
    })
    render(<App />)
    await user.click(screen.getByRole('button', { name: 'TeslaCamフォルダを選ぶ' }))
    expect(await screen.findByText('フォルダの読み取りを完了できませんでした。')).toBeVisible()
  })

  it('loads a fallback directory after the native picker is unsupported', async () => {
    const user = userEvent.setup()
    const click = vi.spyOn(HTMLInputElement.prototype, 'click')
    Object.defineProperty(window, 'showDirectoryPicker', { configurable: true, value: undefined })
    render(<App />)
    const fallbackInput = screen.getByLabelText('TeslaCamフォルダを選ぶ（互換モード）')
    const clip = new File(['clip'], '2026-08-20_20-45-00-front.mp4', { type: 'video/mp4' })
    Object.defineProperty(clip, 'webkitRelativePath', { value: 'RecentClips/2026-08-20_20-45-00-front.mp4' })
    await user.click(screen.getByRole('button', { name: 'TeslaCamフォルダを選ぶ' }))
    fireEvent.change(fallbackInput, { target: { files: [clip] } })
    await waitFor(() => expect(screen.getByRole('button', { name: '別のフォルダを選ぶ' })).toBeVisible())
    const fallbackClickCount = click.mock.calls.length
    await user.click(screen.getByRole('button', { name: '別のフォルダを選ぶ' }))
    expect(click.mock.calls.length).toBeGreaterThan(fallbackClickCount)
  })

  it('clears the fallback input before a folder is selected again', () => {
    render(<App />)
    const fallbackInput = screen.getByLabelText('TeslaCamフォルダを選ぶ（互換モード）')
    Object.defineProperty(fallbackInput, 'value', { configurable: true, value: 'previous-folder', writable: true })
    fireEvent.click(fallbackInput)
    expect(fallbackInput).toHaveValue('')
  })

  it('collapses encrypted-file notice after a local library scan and can recover from invalid input', async () => {
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
              yield { kind: 'file', name: 'hidden.mp4', getFile: async () => new File(['clip'], 'hidden.mp4', { type: 'video/mp4' }) }
            },
          }
        },
      }),
    })
    render(<App />)
    await user.click(screen.getByRole('button', { name: 'TeslaCamフォルダを選ぶ' }))
    await waitFor(() => expect(screen.getByLabelText('読み込み元')).toHaveTextContent('Private Drive'))
    const warningSummary = screen.getByText('読み込みの注意')
    const warnings = warningSummary.closest('details')
    if (warnings === null) {
      throw new Error('読み込みの注意がdetails要素ではありません')
    }
    expect(warnings).not.toHaveAttribute('open')
    expect(screen.getByText(/暗号化されたファイル 1 件/)).not.toBeVisible()
    await user.click(warningSummary)
    expect(warnings).toHaveAttribute('open')
    expect(screen.getByText(/暗号化されたファイル 1 件/)).toBeVisible()
    expect(screen.getByRole('button', { name: 'すべて1' })).toBeVisible()
    const fallbackInput = screen.getByLabelText('TeslaCamフォルダを選ぶ（互換モード）')
    const invalidFile = new File(['clip'], 'clip.mp4')
    Object.defineProperty(invalidFile, 'webkitRelativePath', { value: '' })
    fireEvent.change(fallbackInput, { target: { files: [invalidFile] } })
    await waitFor(() => expect(screen.getByText('TeslaCamフォルダを確認できませんでした。')).toBeVisible())
    await user.click(screen.getByRole('button', { name: 'もう一度選ぶ' }))
    expect(screen.getByRole('button', { name: '別のフォルダを選ぶ' })).toBeVisible()
  })
})
