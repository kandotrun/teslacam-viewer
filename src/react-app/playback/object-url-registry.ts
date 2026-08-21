type UrlEntry = {
  readonly url: string
  readonly referenceCount: number
}

export class ObjectUrlRegistry {
  private readonly entries = new Map<File, UrlEntry>()

  acquire(file: File): string {
    const entry = this.entries.get(file)
    if (entry !== undefined) {
      this.entries.set(file, {
        url: entry.url,
        referenceCount: entry.referenceCount + 1,
      })
      return entry.url
    }

    const url = URL.createObjectURL(file)
    this.entries.set(file, { url, referenceCount: 1 })
    return url
  }

  release(file: File): void {
    const entry = this.entries.get(file)
    if (entry === undefined) return

    if (entry.referenceCount > 1) {
      this.entries.set(file, {
        url: entry.url,
        referenceCount: entry.referenceCount - 1,
      })
      return
    }

    this.entries.delete(file)
    URL.revokeObjectURL(entry.url)
  }

  dispose(): void {
    for (const entry of this.entries.values()) URL.revokeObjectURL(entry.url)
    this.entries.clear()
  }
}
