import type { LocalFileRef } from './types'
import { ensureDirectoryDepth, ensureScanEntryCount } from './scan-limits'

type DirectoryInputFiles = Iterable<File> & { readonly length: number }

function normalizedRelativePath(relativePath: string): string | null {
  const normalized = relativePath.replaceAll('\\', '/')
  if (normalized.startsWith('/') || /^[A-Za-z]:/u.test(normalized)) return null
  const parts = normalized.split('/')
  if (
    parts.some(
      (part) =>
        part === '' ||
        part === '.' ||
        part === '..' ||
        part.startsWith('.') ||
        /^[A-Za-z]:$/u.test(part),
    )
  ) {
    return null
  }
  ensureDirectoryDepth(parts.length - 2)
  return parts.join('/')
}

export function filesFromDirectoryInput(
  files: DirectoryInputFiles,
): readonly LocalFileRef[] {
  ensureScanEntryCount(files.length)
  return [...files].flatMap((file) => {
    const relativePath = normalizedRelativePath(file.webkitRelativePath)
    return relativePath === null
      ? []
      : [
          {
            name: file.name,
            relativePath,
            size: file.size,
            type: file.type,
            getFile: async () => file,
          },
        ]
  })
}
