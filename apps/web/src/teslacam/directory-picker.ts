import type { LocalFileRef, ScanProgress } from './types'
import { createYieldBudget, type YieldBudget } from './cooperative-yield'
import { ensureDirectoryDepth, ensureScanEntryCount } from './scan-limits'
import { createTaskPool, type TaskOutcome, type TaskPool } from './task-pool'

export type DirectorySelectionResult =
  | {
      readonly status: 'selected'
      readonly files: readonly LocalFileRef[]
      readonly folderName: string
    }
  | { readonly status: 'cancelled' }
  | { readonly status: 'denied' }
  | { readonly status: 'unsupported' }

export type DirectoryRequestResult =
  | { readonly status: 'picked'; readonly root: ScanDirectoryHandle }
  | { readonly status: 'cancelled' }
  | { readonly status: 'denied' }
  | { readonly status: 'unsupported' }

/** The parts of `FileSystemHandle` the walk needs; real handles and test doubles both fit. */
type HandleLike = { readonly kind: string; readonly name: string }
type ScanFileHandle = HandleLike & { getFile(): Promise<File> }
export type ScanDirectoryHandle = HandleLike & { values(): AsyncIterable<HandleLike> }

type ScanOptions = { readonly signal?: AbortSignal | undefined }

type PendingFile = {
  readonly name: string
  readonly relativePath: string
  readonly handle: ScanFileHandle
  readonly snapshot: Promise<TaskOutcome<File>>
}

type ScanState = {
  readonly files: PendingFile[]
  readonly report: (progress: ScanProgress) => void
  readonly snapshots: TaskPool
  readonly budget: YieldBudget
  readonly signal: AbortSignal | undefined
  processed: number
}

/** File snapshots (for the byte size) requested at once; the walk itself never waits for them. */
export const DIRECTORY_SNAPSHOT_CONCURRENCY = 16

function isHidden(name: string): boolean {
  return name.startsWith('.')
}

function isFileHandle(entry: HandleLike): entry is ScanFileHandle {
  return entry.kind === 'file' && 'getFile' in entry && typeof entry.getFile === 'function'
}

function isDirectoryHandle(entry: HandleLike): entry is ScanDirectoryHandle {
  return entry.kind === 'directory' && 'values' in entry && typeof entry.values === 'function'
}

function throwIfAborted(signal: AbortSignal | undefined): void {
  if (signal?.aborted === true) throw new DOMException('Scan aborted', 'AbortError')
}

async function countEntry(state: ScanState): Promise<void> {
  state.processed += 1
  ensureScanEntryCount(state.processed)
  if (state.processed % 250 === 0) {
    state.report({ processed: state.processed, total: state.processed })
  }
  await state.budget.checkpoint()
  throwIfAborted(state.signal)
}

function requestSnapshot(state: ScanState, handle: ScanFileHandle): Promise<TaskOutcome<File>> {
  return state.snapshots.run(() => handle.getFile()).then(
    (value): TaskOutcome<File> => ({ status: 'fulfilled', value }),
    (reason: unknown): TaskOutcome<File> => ({ status: 'rejected', reason }),
  )
}

async function scanDirectory(
  directory: ScanDirectoryHandle,
  path: readonly string[],
  state: ScanState,
  depth: number,
): Promise<void> {
  ensureDirectoryDepth(depth)
  for await (const entry of directory.values()) {
    await countEntry(state)
    if (isHidden(entry.name)) continue
    const entryPath = [...path, entry.name]
    if (isFileHandle(entry)) {
      state.files.push({
        name: entry.name,
        relativePath: entryPath.join('/'),
        handle: entry,
        snapshot: requestSnapshot(state, entry),
      })
    } else if (isDirectoryHandle(entry)) {
      await scanDirectory(entry, entryPath, state, depth + 1)
    }
  }
}

/** A reference whose `File` is fetched from the handle on first use (playback, export, thumbnails). */
function lazyFileRef(pending: PendingFile, snapshot: File): LocalFileRef {
  let file: Promise<File> | null = null
  return {
    name: pending.name,
    relativePath: pending.relativePath,
    size: snapshot.size,
    type: snapshot.type,
    getFile: () => {
      file ??= pending.handle.getFile().catch((error: unknown) => {
        file = null
        throw error
      })
      return file
    },
  }
}

async function resolveFiles(pendingFiles: readonly PendingFile[]): Promise<readonly LocalFileRef[]> {
  const snapshots = await Promise.all(pendingFiles.map((pending) => pending.snapshot))
  return pendingFiles.map((pending, index) => {
    const snapshot = snapshots[index]
    if (snapshot === undefined) throw new Error('File snapshot is missing')
    if (snapshot.status === 'rejected') throw snapshot.reason
    return lazyFileRef(pending, snapshot.value)
  })
}

function selectionError(error: unknown): { readonly status: 'cancelled' } | { readonly status: 'denied' } | null {
  if (!(error instanceof DOMException)) return null
  if (error.name === 'AbortError') return { status: 'cancelled' }
  if (error.name === 'NotAllowedError') return { status: 'denied' }
  return null
}

/** Opens the native folder picker only; nothing is read until `scanTeslaCamDirectory`. */
export async function requestTeslaCamDirectory(): Promise<DirectoryRequestResult> {
  const picker = globalThis.showDirectoryPicker
  if (picker === undefined) return { status: 'unsupported' }
  try {
    return { status: 'picked', root: await picker({ mode: 'read' }) }
  } catch (error) {
    const result = selectionError(error)
    if (result !== null) return result
    throw error
  }
}

/**
 * Walks the chosen folder (§1.3 of the parity spec). Directory entries are enumerated without waiting for
 * file snapshots; the snapshots that provide each file's byte size are read in parallel in a bounded pool.
 */
export async function scanTeslaCamDirectory(
  root: ScanDirectoryHandle,
  report: (progress: ScanProgress) => void,
  options: ScanOptions = {},
): Promise<{ readonly status: 'selected'; readonly files: readonly LocalFileRef[]; readonly folderName: string }> {
  const state: ScanState = {
    files: [],
    report,
    snapshots: createTaskPool(DIRECTORY_SNAPSHOT_CONCURRENCY),
    budget: createYieldBudget(),
    signal: options.signal,
    processed: 0,
  }
  try {
    throwIfAborted(options.signal)
    await scanDirectory(root, [root.name], state, 0)
    report({ processed: state.processed, total: state.processed })
    const files = await resolveFiles(state.files)
    throwIfAborted(options.signal)
    return { status: 'selected', files, folderName: root.name }
  } finally {
    state.snapshots.cancel()
  }
}

export async function pickTeslaCamDirectory(
  report: (progress: ScanProgress) => void,
): Promise<DirectorySelectionResult> {
  const request = await requestTeslaCamDirectory()
  if (request.status !== 'picked') return request
  return scanTeslaCamDirectory(request.root, report)
}
