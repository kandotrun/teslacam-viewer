import userEvent from '@testing-library/user-event'

import { filesFromDirectoryInput } from './directory-input'

function inputFiles(): HTMLInputElement {
  const input = document.createElement('input')
  input.type = 'file'
  input.multiple = true
  return input
}

function fileWithPath(name: string, relativePath: string): File {
  const file = new File(['clip'], name, { type: 'video/mp4' })
  Object.defineProperty(file, 'webkitRelativePath', { value: relativePath })
  return file
}

describe('filesFromDirectoryInput', () => {
  it('normalizes safe paths and preserves the original File identity', async () => {
    const source = fileWithPath(
      '2026-08-21_14-30-45-front.mp4',
      'TeslaCam\\SavedClips\\saved-a\\2026-08-21_14-30-45-front.mp4',
    )
    const input = inputFiles()
    await userEvent.upload(input, source)
    expect(input.files).not.toBeNull()
    if (input.files === null) return

    const files = filesFromDirectoryInput(input.files)

    expect(files).toHaveLength(1)
    expect(files[0]?.relativePath).toBe(
      'TeslaCam/SavedClips/saved-a/2026-08-21_14-30-45-front.mp4',
    )
    await expect(files[0]?.getFile()).resolves.toBe(source)
  })

  it('keeps a directly selected category recognizable to Task 2', async () => {
    const source = fileWithPath(
      '2026-08-21_14-30-45-front.mp4',
      'SavedClips/saved-a/2026-08-21_14-30-45-front.mp4',
    )
    const input = inputFiles()
    await userEvent.upload(input, source)
    expect(input.files).not.toBeNull()
    if (input.files === null) return

    const files = filesFromDirectoryInput(input.files)

    expect(files[0]?.relativePath).toBe(
      'SavedClips/saved-a/2026-08-21_14-30-45-front.mp4',
    )
  })

  it('skips hidden and unsafe paths without discarding safe files', async () => {
    const safe = fileWithPath(
      '2026-08-21_14-30-45-front.mp4',
      'TeslaCam/RecentClips/2026-08-21_14-30-45-front.mp4',
    )
    const hidden = fileWithPath('.DS_Store', 'TeslaCam/.meta/.DS_Store')
    const traversal = fileWithPath('bad.mp4', 'TeslaCam/SavedClips/../bad.mp4')
    const absolute = fileWithPath('bad.mp4', '/TeslaCam/SavedClips/bad.mp4')
    const drive = fileWithPath('bad.mp4', 'C:\\TeslaCam\\SavedClips\\bad.mp4')
    const empty = fileWithPath('bad.mp4', 'TeslaCam//SavedClips/bad.mp4')
    const dot = fileWithPath('bad.mp4', 'TeslaCam/./SavedClips/bad.mp4')
    const input = inputFiles()
    await userEvent.upload(input, [safe, hidden, traversal, absolute, drive, empty, dot])
    expect(input.files).not.toBeNull()
    if (input.files === null) return

    const files = filesFromDirectoryInput(input.files)

    expect(files).toHaveLength(1)
    await expect(files[0]?.getFile()).resolves.toBe(safe)
  })
})
