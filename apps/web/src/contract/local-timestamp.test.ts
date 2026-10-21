// @vitest-environment node
import { describe } from 'vitest'

import {
  formatFilenameTimestampAtOffset,
  parseFilenameLocalTimestamp,
  parseMetadataLocalTimestamp,
} from '../react-app/local-timestamp'
import { encodeNumber } from './adapters'
import { defineFixture, type FixtureCase } from './fixture-file'

function filenameCase(name: string, value: string): FixtureCase {
  return { name: `filename ${name}`, fn: 'parseFilenameLocalTimestamp', input: { value }, expected: parseFilenameLocalTimestamp(value) }
}

function metadataCase(name: string, value: string): FixtureCase {
  return { name: `metadata ${name}`, fn: 'parseMetadataLocalTimestamp', input: { value }, expected: parseMetadataLocalTimestamp(value) }
}

function offsetCase(name: string, value: string, offsetSeconds: number): FixtureCase {
  return {
    name: `offset ${name}`,
    fn: 'formatFilenameTimestampAtOffset',
    input: { value, offsetSeconds: encodeNumber(offsetSeconds) },
    expected: formatFilenameTimestampAtOffset(value, offsetSeconds),
  }
}

describe('contract: local-timestamp', () => {
  defineFixture({
    file: 'local-timestamp',
    module: 'local-timestamp',
    source: ['apps/web/src/react-app/local-timestamp.ts'],
    cases: () => [
      filenameCase('end of day', '2026-08-24_23-59-59'),
      filenameCase('start of day', '2026-08-24_00-00-00'),
      filenameCase('epoch', '0001-01-01_00-00-00'),
      filenameCase('year 0000', '0000-01-01_00-00-00'),
      filenameCase('year 9999', '9999-12-31_23-59-59'),
      filenameCase('leap day 2028', '2028-02-29_12-00-00'),
      filenameCase('leap day 2000', '2000-02-29_12-00-00'),
      filenameCase('non-leap 2026', '2026-02-29_12-00-00'),
      filenameCase('non-leap 1900', '1900-02-29_12-00-00'),
      filenameCase('March after leap day', '2028-03-01_00-00-00'),
      filenameCase('December 31', '2026-12-31_23-59-59'),
      filenameCase('April 31', '2026-04-31_00-00-00'),
      filenameCase('month 00', '2026-00-10_00-00-00'),
      filenameCase('month 13', '2026-13-10_00-00-00'),
      filenameCase('day 00', '2026-08-00_00-00-00'),
      filenameCase('hour 24', '2026-08-24_24-00-00'),
      filenameCase('minute 60', '2026-08-24_12-60-00'),
      filenameCase('second 60', '2026-08-24_12-00-60'),
      filenameCase('metadata format', '2026-08-24T12:00:30'),
      filenameCase('trailing newline', '2026-08-24_12-00-30\n'),
      filenameCase('short year', '226-08-24_12-00-30'),
      filenameCase('full-width digits', '２０２６-08-24_12-00-30'),
      filenameCase('empty', ''),
      metadataCase('plain', '2026-08-24T12:00:30'),
      metadataCase('utc suffix', '2026-08-24T12:00:30Z'),
      metadataCase('milliseconds', '2026-08-24T12:00:30.250Z'),
      metadataCase('one fraction digit', '2026-08-24T12:00:30.2'),
      metadataCase('two fraction digits', '2026-08-24T12:00:30.25'),
      metadataCase('three fraction digits', '2026-08-24T12:00:05.900'),
      metadataCase('fraction 999', '2026-08-24T23:59:59.999Z'),
      metadataCase('four fraction digits', '2026-08-24T12:00:30.2500'),
      metadataCase('empty fraction', '2026-08-24T12:00:30.'),
      metadataCase('comma fraction', '2026-08-24T12:00:30,250'),
      metadataCase('offset +09:00', '2026-08-24T12:00:30+09:00'),
      metadataCase('lower-case z', '2026-08-24T12:00:30z'),
      metadataCase('lower-case t', '2026-08-24t12:00:30'),
      metadataCase('space separator', '2026-08-24 12:00:30'),
      metadataCase('filename format', '2026-08-24_12-00-30'),
      metadataCase('missing seconds', '2026-08-24T12:00'),
      metadataCase('invalid date', '2026-02-30T12:00:30'),
      metadataCase('hour 24', '2026-08-24T24:00:00'),
      metadataCase('leap day', '2028-02-29T00:00:00Z'),
      metadataCase('double Z', '2026-08-24T12:00:30ZZ'),
      metadataCase('invalid', 'invalid'),
      metadataCase('empty', ''),
      offsetCase('year boundary', '2026-12-31_23-59-45', 30),
      offsetCase('into leap day', '2028-02-28_23-59-59', 1),
      offsetCase('negative into previous year', '2026-01-01_00-00-10', -20),
      offsetCase('zero', '2026-08-24_12-00-00', 0),
      offsetCase('sentry pre-roll', '2026-08-24_12-01-00', 5),
      offsetCase('one day', '2026-08-24_12-00-00', 86_400),
      offsetCase('leap year length', '2028-01-01_00-00-00', 366 * 86_400),
      offsetCase('century non-leap', '2100-02-28_23-59-59', 1),
      offsetCase('to epoch', '0001-01-01_00-00-10', -10),
      offsetCase('before epoch', '0001-01-01_00-00-00', -1),
      offsetCase('past year 9999', '9999-12-31_23-59-59', 1),
      offsetCase('fractional offset', '2026-08-24_12-00-00', 1.5),
      offsetCase('NaN offset', '2026-08-24_12-00-00', Number.NaN),
      offsetCase('infinite offset', '2026-08-24_12-00-00', Number.POSITIVE_INFINITY),
      offsetCase('negative infinite offset', '2026-08-24_12-00-00', Number.NEGATIVE_INFINITY),
      offsetCase('unsafe integer result', '2026-08-24_12-00-00', 2 ** 53),
      offsetCase('invalid timestamp', '2026-02-29_12-00-00', 30),
      offsetCase('metadata timestamp', '2026-08-24T12:00:00', 30),
    ],
  })
})
