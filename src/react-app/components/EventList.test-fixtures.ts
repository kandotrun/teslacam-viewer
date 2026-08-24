import type { LocalFileRef, TeslaCamEvent } from '../../teslacam/types'

export const events: readonly TeslaCamEvent[] = [
  {
    id: 'sentry-newer',
    category: 'sentry',
    timestamp: '2026-08-20_20-45-00',
    metadata: { timestamp: '2026-08-20T20:45:15' },
    segments: [{ timestamp: '2026-08-20_20-45-00', cameras: { front: fileRef('front.mp4') } }],
  },
  {
    id: 'recent-older',
    category: 'recent',
    timestamp: '2026-08-19_08-30-00',
    segments: [{ timestamp: '2026-08-19_08-30-00', cameras: { front: fileRef('front.mp4') } }],
  },
]

export function fileRef(name: string): LocalFileRef {
  return {
    name,
    relativePath: `RecentClips/${name}`,
    size: 12,
    type: 'video/mp4',
    getFile: async () => new File(['clip'], name, { type: 'video/mp4' }),
  }
}

export function firstEvent(): TeslaCamEvent {
  const event = events.at(0)
  if (event === undefined) throw new Error('event fixture is missing')
  return event
}

export function pendingFileRequest(): {
  readonly promise: Promise<File>
  readonly resolve: (file: File) => void
  readonly reject: (reason?: unknown) => void
} {
  let resolveFile: (file: File) => void = () => undefined
  let rejectFile: (reason?: unknown) => void = () => undefined
  const promise = new Promise<File>((resolve, reject) => {
    resolveFile = resolve
    rejectFile = reject
  })
  return { promise, resolve: resolveFile, reject: rejectFile }
}
