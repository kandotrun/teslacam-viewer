import type { RefObject } from 'react'

type DirectoryInputProps = {
  readonly inputRef: RefObject<HTMLInputElement | null>
  readonly onDirectoryChange: (files: FileList | null) => void
  /** With the native picker the input is only clicked programmatically, so it stays out of the Tab order. */
  readonly nativePickerAvailable: boolean
}

export function DirectoryInput({ inputRef, onDirectoryChange, nativePickerAvailable }: DirectoryInputProps) {
  return (
    <input
      aria-hidden={nativePickerAvailable || undefined}
      aria-label="TeslaCamフォルダを選ぶ（互換モード）"
      className="directory-input"
      ref={inputRef}
      tabIndex={nativePickerAvailable ? -1 : undefined}
      type="file"
      webkitdirectory=""
      onClick={(event) => {
        event.currentTarget.value = ''
      }}
      onChange={(event) => onDirectoryChange(event.currentTarget.files)}
    />
  )
}
