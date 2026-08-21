import 'react'

declare module 'react' {
  interface InputHTMLAttributes<T> {
    readonly webkitdirectory?: string
  }
}

declare global {
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

  var showDirectoryPicker:
    | ((
        options?: DirectoryPickerOptions,
      ) => Promise<FileSystemDirectoryHandle>)
    | undefined
}

export {}
