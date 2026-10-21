export const MAX_SCAN_ENTRIES = 100_000
export const MAX_DIRECTORY_DEPTH = 8
export const MAX_EVENT_METADATA_SIZE = 1024 * 1024
export const MAX_METADATA_STRING_LENGTH = 2048

type ScanLimitKind =
  | 'entry-count'
  | 'directory-depth'
  | 'metadata-size'
  | 'metadata-string'

export class ScanLimitError extends Error {
  readonly name = 'ScanLimitError'

  constructor(readonly kind: ScanLimitKind) {
    super('Selected folder exceeds a safe local scan limit')
  }
}

export function ensureScanEntryCount(count: number): void {
  if (count > MAX_SCAN_ENTRIES) throw new ScanLimitError('entry-count')
}

export function ensureDirectoryDepth(depth: number): void {
  if (depth > MAX_DIRECTORY_DEPTH) throw new ScanLimitError('directory-depth')
}

export function ensureMetadataSize(size: number): void {
  if (size > MAX_EVENT_METADATA_SIZE) throw new ScanLimitError('metadata-size')
}
