import {
  formatFilenameTimestampAtOffset,
  parseFilenameLocalTimestamp,
  parseMetadataLocalTimestamp,
} from './local-timestamp'

describe('parseFilenameLocalTimestamp', () => {
  it('parses a valid filename timestamp into local calendar fields', () => {
    const timestamp = parseFilenameLocalTimestamp('2026-08-24_23-59-59')

    expect(timestamp).toMatchObject({
      dateKey: '2026-08-24',
      timeLabel: '23:59:59',
      secondsOfDay: 86_399,
    })
  })

  it('rejects invalid calendar dates while accepting valid leap days', () => {
    expect(parseFilenameLocalTimestamp('2026-02-29_12-00-00')).toBeNull()
    expect(parseFilenameLocalTimestamp('2028-02-29_12-00-00')).not.toBeNull()
  })

  it('rejects invalid time values and non-filename formats', () => {
    expect(parseFilenameLocalTimestamp('2026-08-24_24-00-00')).toBeNull()
    expect(parseFilenameLocalTimestamp('2026-08-24T12:00:30')).toBeNull()
  })
})

describe('parseMetadataLocalTimestamp', () => {
  it('retains milliseconds from Tesla metadata without timezone conversion', () => {
    const timestamp = parseMetadataLocalTimestamp('2026-08-24T12:00:30.250Z')

    expect(timestamp?.millisecond).toBe(250)
    expect(timestamp?.timeLabel).toBe('12:00:30')
  })

  it('rejects timezone offsets and non-metadata formats', () => {
    expect(parseMetadataLocalTimestamp('2026-08-24T12:00:30+09:00')).toBeNull()
    expect(parseMetadataLocalTimestamp('2026-08-24_12-00-30')).toBeNull()
  })
})

describe('formatFilenameTimestampAtOffset', () => {
  it('carries a positive offset across a year boundary', () => {
    expect(formatFilenameTimestampAtOffset('2026-12-31_23-59-45', 30)).toBe(
      '2027-01-01 00:00:15',
    )
  })

  it('returns null for invalid filename timestamps', () => {
    expect(formatFilenameTimestampAtOffset('2026-02-29_12-00-00', 30)).toBeNull()
  })
})
