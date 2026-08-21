interface DirectoryPickerOptions {
  readonly mode?: 'read' | 'readwrite'
}

interface File {
  readonly webkitRelativePath: string
}

interface Window {
  showDirectoryPicker?: (
    options?: DirectoryPickerOptions,
  ) => Promise<FileSystemDirectoryHandle>
}
