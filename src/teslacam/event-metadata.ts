import { z } from 'zod'

import type { EventMetadata } from './types'

const eventMetadataSchema = z
  .object({
    timestamp: z.string(),
    city: z.string().optional(),
    street: z.string().optional(),
    est_lat: z.string().optional(),
    est_lon: z.string().optional(),
    reason: z.string().optional(),
    camera: z.string().optional(),
  })
  .passthrough()

export function parseEventMetadata(value: unknown): EventMetadata | null {
  const parsed = eventMetadataSchema.safeParse(value)
  if (!parsed.success) return null

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
