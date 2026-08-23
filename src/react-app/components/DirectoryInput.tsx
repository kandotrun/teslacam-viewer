import type { RefObject } from 'react'

type DirectoryInputProps = {
  readonly inputRef: RefObject<HTMLInputElement | null>
  readonly onDirectoryChange: (files: FileList | null) => void
}

export function DirectoryInput({ inputRef, onDirectoryChange }: DirectoryInputProps) {
  return (
    <input
      aria-label="TeslaCamフォルダを選ぶ（互換モード）"
      className="directory-input"
      ref={inputRef}
      type="file"
      webkitdirectory=""
      onClick={(event) => {
        event.currentTarget.value = ''
      }}
      onChange={(event) => onDirectoryChange(event.currentTarget.files)}
    />
  )
}
