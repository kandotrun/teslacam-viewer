import { z } from 'zod'

import {
  ensureMetadataSize,
  MAX_METADATA_STRING_LENGTH,
  ScanLimitError,
} from './scan-limits'
import type { EventMetadata, LocalFileRef } from './types'

const metadataStringSchema = z.string().max(MAX_METADATA_STRING_LENGTH)

const eventMetadataSchema = z
  .object({
    timestamp: metadataStringSchema,
    city: metadataStringSchema.optional(),
    street: metadataStringSchema.optional(),
    est_lat: metadataStringSchema.optional(),
    est_lon: metadataStringSchema.optional(),
    reason: metadataStringSchema.optional(),
    camera: metadataStringSchema.optional(),
  })
  .passthrough()

export function parseEventMetadata(value: unknown): EventMetadata | null {
  const parsed = eventMetadataSchema.safeParse(value)
  if (!parsed.success) {
    if (parsed.error.issues.some((issue) => issue.code === 'too_big')) {
      throw new ScanLimitError('metadata-string')
    }
    return null
  }

  const { timestamp, city, street, est_lat, est_lon, reason, camera } =
    parsed.data
  return {
    timestamp,
    ...(city === undefined ? {} : { city }),
    ...(street === undefined ? {} : { street }),
    ...(est_lat === undefined ? {} : { est_lat }),
    ...(est_lon === undefined ? {} : { est_lon }),
    ...(reason === undefined ? {} : { reason }),
    ...(camera === undefined ? {} : { camera }),
  }
}

export async function readEventMetadata(
  file: LocalFileRef,
): Promise<EventMetadata | null> {
  try {
    ensureMetadataSize(file.size)
    const snapshot = await file.getFile()
    ensureMetadataSize(snapshot.size)
    const contents = await snapshot.text()
    return parseEventMetadata(JSON.parse(contents))
  } catch (error) {
    if (error instanceof ScanLimitError) throw error
    if (error instanceof Error) return null
    throw error
  }
}
