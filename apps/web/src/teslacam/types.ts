export const cameraIds = [
  'front',
  'back',
  'left_pillar',
  'left_repeater',
  'right_pillar',
  'right_repeater',
] as const

export type CameraId = (typeof cameraIds)[number]

export type ClipCategory = 'recent' | 'saved' | 'sentry'

export type LocalFileRef = {
  readonly name: string
  readonly relativePath: string
  readonly size: number
  readonly type: string
  readonly getFile: () => Promise<File>
}

export type ParsedClipName = {
  readonly timestamp: string
  readonly camera: CameraId
}

export type EventMetadata = {
  readonly timestamp: string
  readonly city?: string
  readonly street?: string
  readonly est_lat?: string
  readonly est_lon?: string
  readonly reason?: string
  readonly camera?: string
}

export type TeslaCamSegment = {
  readonly timestamp: string
  readonly cameras: Readonly<Partial<Record<CameraId, LocalFileRef>>>
}

export type TeslaCamEvent = {
  readonly id: string
  readonly category: ClipCategory
  readonly timestamp: string
  readonly segments: readonly TeslaCamSegment[]
  readonly representativeFile?: LocalFileRef
  readonly thumbnailFile?: LocalFileRef
  readonly metadata?: EventMetadata
}

export type ScanWarning = {
  readonly kind:
    | 'empty-media-file'
    | 'invalid-metadata-file'
    | 'unknown-file'
    | 'encrypted-file'
  readonly count: number
}

export type ScanProgress = {
  readonly processed: number
  readonly total: number
}

export type TeslaCamLibrary = {
  readonly events: readonly TeslaCamEvent[]
  readonly eventsByCategory: Readonly<
    Record<ClipCategory, readonly TeslaCamEvent[]>
  >
  readonly warnings: readonly ScanWarning[]
}
