type SyntheticFileOptions = {
  readonly contents?: string
  readonly size?: number
  readonly type?: string
}

function fileNameFromPath(relativePath: string): string {
  const name = relativePath.split('/').at(-1)
  return name === undefined ? relativePath : name
}

export function createLocalFileRef(
  relativePath: string,
  options: SyntheticFileOptions = {},
) {
  const name = fileNameFromPath(relativePath)
  const contents = options.contents ?? 'synthetic-file'
  const type = options.type ?? 'video/mp4'
  const file = new File([contents], name, { type })

  return {
    name,
    relativePath,
    size: options.size ?? file.size,
    type,
    getFile: async () => file,
  }
}
