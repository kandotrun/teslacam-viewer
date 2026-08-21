import type { LocalFileRef, ScanProgress } from './types'

export type DirectorySelectionResult =
  | {
      readonly status: 'selected'
      readonly files: readonly LocalFileRef[]
      readonly folderName: string
    }
  | { readonly status: 'cancelled' }
  | { readonly status: 'denied' }
  | { readonly status: 'unsupported' }

type ScanState = {
  readonly files: LocalFileRef[]
  readonly report: (progress: ScanProgress) => void
  processed: number
}

function yieldToBrowser(): Promise<void> {
  return new Promise((resolve) => {
    globalThis.setTimeout(resolve, 0)
  })
}

function isHidden(name: string): boolean {
  return name.startsWith('.')
}

function isFileHandle(entry: FileSystemHandle): entry is FileSystemFileHandle {
  return entry.kind === 'file'
}

async function countEntry(state: ScanState): Promise<void> {
  state.processed += 1
  if (state.processed % 250 === 0) {
    state.report({ processed: state.processed, total: state.processed })
    await yieldToBrowser()
  }
}

async function scanDirectory(
  directory: FileSystemDirectoryHandle,
  path: readonly string[],
  state: ScanState,
): Promise<void> {
  for await (const entry of directory.values()) {
    if (isHidden(entry.name)) continue
    await countEntry(state)
    const relativePath = [...path, entry.name].join('/')
    if (isFileHandle(entry)) {
      const file = await entry.getFile()
      state.files.push({
        name: file.name,
        relativePath,
        size: file.size,
        type: file.type,
        getFile: async () => file,
      })
    } else {
      await scanDirectory(entry, [...path, entry.name], state)
    }
  }
}

function selectionError(error: unknown): DirectorySelectionResult | null {
  if (!(error instanceof DOMException)) return null
  if (error.name === 'AbortError') return { status: 'cancelled' }
  if (error.name === 'NotAllowedError') return { status: 'denied' }
  return null
}

export async function pickTeslaCamDirectory(
  report: (progress: ScanProgress) => void,
): Promise<DirectorySelectionResult> {
  const picker = globalThis.showDirectoryPicker
  if (picker === undefined) return { status: 'unsupported' }

  let root: FileSystemDirectoryHandle
  try {
    root = await picker({ mode: 'read' })
  } catch (error) {
    const result = selectionError(error)
    if (result !== null) return result
    throw error
  }

  const state: ScanState = { files: [], report, processed: 0 }
  await scanDirectory(root, [root.name], state)
  report({ processed: state.processed, total: state.processed })
  return { status: 'selected', files: state.files, folderName: root.name }
}
