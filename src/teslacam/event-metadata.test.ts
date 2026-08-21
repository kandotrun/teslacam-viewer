import { parseEventMetadata } from './event-metadata'

describe('parseEventMetadata', () => {
  it('retains every known string field from valid metadata', () => {
    const result = parseEventMetadata({
      timestamp: '2026-08-21_14-30-45',
      city: 'Sample city',
      street: 'Sample street',
      est_lat: '35.0000',
      est_lon: '139.0000',
      reason: 'sentry_aware_object_detection',
      camera: 'left_repeater',
    })

    expect(result).toEqual({
      timestamp: '2026-08-21_14-30-45',
      city: 'Sample city',
      street: 'Sample street',
      est_lat: '35.0000',
      est_lon: '139.0000',
      reason: 'sentry_aware_object_detection',
      camera: 'left_repeater',
    })
  })

  it('omits absent optional fields and ignores unknown metadata fields', () => {
    const result = parseEventMetadata({
      timestamp: '2026-08-21_14-30-45',
      untrusted_extra: 'discarded',
    })

    expect(result).toEqual({ timestamp: '2026-08-21_14-30-45' })
  })

  it.each([
    null,
    [],
    42,
    { timestamp: 42 },
    { timestamp: '2026-08-21_14-30-45', reason: 42 },
  ])('returns null instead of coercing invalid metadata', (value) => {
    expect(parseEventMetadata(value)).toBeNull()
  })
})
