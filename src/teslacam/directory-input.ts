import type { LocalFileRef } from './types'

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
  return parts.join('/')
}

export function filesFromDirectoryInput(files: FileList): readonly LocalFileRef[] {
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
