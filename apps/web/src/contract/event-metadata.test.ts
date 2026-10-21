// @vitest-environment node
import { describe } from 'vitest'

import { parseEventMetadata, readEventMetadata } from '../teslacam/event-metadata'
import { MAX_EVENT_METADATA_SIZE, MAX_METADATA_STRING_LENGTH, ScanLimitError } from '../teslacam/scan-limits'
import type { EventMetadata, LocalFileRef } from '../teslacam/types'
import { expandString, generated, metadataJson, type GeneratedString } from './adapters'
import { defineFixture, type FixtureCase, type JsonValue } from './fixture-file'

const knownFields = ['timestamp', 'city', 'street', 'est_lat', 'est_lon', 'reason', 'camera'] as const

type GeneratedJson =
  | JsonValue
  | GeneratedString
  | readonly GeneratedJson[]
  | { readonly [key: string]: GeneratedJson }

function isGeneratedString(value: GeneratedJson): value is GeneratedString {
  return typeof value === 'object' && value !== null && !Array.isArray(value) && '$repeat' in value && 'count' in value
}

function isGeneratedArray(value: GeneratedJson): value is readonly GeneratedJson[] {
  return Array.isArray(value)
}

/** Expands every `GeneratedString` inside a JSON input. */
function expandJson(value: GeneratedJson): unknown {
  if (value === null || typeof value !== 'object') return value
  if (isGeneratedString(value)) return expandString(value)
  if (isGeneratedArray(value)) return value.map(expandJson)
  return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, expandJson(item)]))
}

function inputJson(value: GeneratedJson): JsonValue {
  if (value === null || typeof value !== 'object') return value
  if (isGeneratedString(value)) return { $repeat: value.$repeat, count: value.count }
  if (isGeneratedArray(value)) return value.map(inputJson)
  return Object.fromEntries(Object.entries(value).map(([key, item]) => [key, inputJson(item)]))
}

function metadataResult(compute: () => EventMetadata | null): JsonValue {
  try {
    const metadata = compute()
    return metadata === null ? { result: 'invalid' } : { result: 'ok', metadata: metadataJson(metadata) }
  } catch (error) {
    if (error instanceof ScanLimitError) return { result: 'scan-limit', kind: error.kind }
    throw error
  }
}

async function asyncMetadataResult(compute: () => Promise<EventMetadata | null>): Promise<JsonValue> {
  try {
    const metadata = await compute()
    return metadata === null ? { result: 'invalid' } : { result: 'ok', metadata: metadataJson(metadata) }
  } catch (error) {
    if (error instanceof ScanLimitError) return { result: 'scan-limit', kind: error.kind }
    throw error
  }
}

function parseCase(name: string, json: GeneratedJson): FixtureCase {
  return {
    name: `parse ${name}`,
    fn: 'parseEventMetadata',
    input: { json: inputJson(json) },
    expected: metadataResult(() => parseEventMetadata(expandJson(json))),
  }
}

type ReadInput =
  | { readonly text: string | GeneratedString; readonly declaredSize?: number }
  | { readonly bytes: readonly number[]; readonly declaredSize?: number }

function inputBytes(input: ReadInput): Uint8Array {
  if ('text' in input) return new TextEncoder().encode(expandString(input.text))
  return Uint8Array.from(input.bytes)
}

function metadataFile(bytes: Uint8Array, declaredSize: number): LocalFileRef {
  const file = new File([new Uint8Array(bytes)], 'event.json', { type: 'application/json' })
  return {
    name: 'event.json',
    relativePath: 'TeslaCam/SentryClips/event/event.json',
    size: declaredSize,
    type: file.type,
    getFile: async () => file,
  }
}

async function readCase(name: string, input: ReadInput): Promise<FixtureCase> {
  const bytes = inputBytes(input)
  const declaredSize = input.declaredSize ?? bytes.length
  const json: JsonValue = 'text' in input
    ? { text: inputJson(input.text), ...(input.declaredSize === undefined ? {} : { declaredSize }) }
    : { base64: Buffer.from(bytes).toString('base64'), ...(input.declaredSize === undefined ? {} : { declaredSize }) }
  return {
    name: `read ${name}`,
    fn: 'readEventMetadata',
    input: json,
    expected: await asyncMetadataResult(() => readEventMetadata(metadataFile(bytes, declaredSize))),
  }
}

const limit = MAX_METADATA_STRING_LENGTH
const detection = '2026-08-20T20:35:30'

function fieldLengthCases(): FixtureCase[] {
  return knownFields.flatMap((field) => [
    parseCase(`${field} at the string limit`, { timestamp: detection, [field]: generated('x', limit) }),
    parseCase(`${field} over the string limit`, { timestamp: detection, [field]: generated('x', limit + 1) }),
  ])
}

describe('contract: event-metadata', () => {
  defineFixture({
    file: 'event-metadata',
    module: 'event-metadata',
    source: ['apps/web/src/teslacam/event-metadata.ts', 'apps/web/src/teslacam/scan-limits.ts'],
    cases: async () => [
      parseCase('all known fields', {
        timestamp: '2026-08-21_14-30-45',
        city: 'Sample city',
        street: 'Sample street',
        est_lat: '35.0000',
        est_lon: '139.0000',
        reason: 'sentry_aware_object_detection',
        camera: 'left_repeater',
      }),
      parseCase('timestamp only', { timestamp: detection }),
      parseCase('unknown fields dropped', { timestamp: detection, untrusted_extra: 'discarded', nested: { deep: [1, 2] } }),
      parseCase('empty strings kept', { timestamp: '', city: '', reason: '' }),
      parseCase('null', null),
      parseCase('array', []),
      parseCase('array with object', [{ timestamp: detection }]),
      parseCase('number', 42),
      parseCase('string', 'timestamp'),
      parseCase('boolean', true),
      parseCase('empty object', {}),
      parseCase('numeric timestamp', { timestamp: 42 }),
      parseCase('null timestamp', { timestamp: null }),
      parseCase('numeric reason', { timestamp: detection, reason: 42 }),
      parseCase('null city', { timestamp: detection, city: null }),
      parseCase('object street', { timestamp: detection, street: { name: 'x' } }),
      parseCase('boolean camera', { timestamp: detection, camera: false }),
      parseCase('unknown field over the string limit', { timestamp: detection, notes: generated('x', limit + 1) }),
      ...fieldLengthCases(),
      parseCase('surrogate pairs at the limit', { timestamp: detection, city: generated('😀', limit / 2) }),
      parseCase('surrogate pairs over the limit', { timestamp: detection, city: generated('😀', limit / 2 + 1) }),
      parseCase('CJK at the limit', { timestamp: detection, street: generated('渋', limit) }),
      parseCase('too long field wins over invalid timestamp', { timestamp: 42, city: generated('x', limit + 1) }),
      parseCase('too long field wins over missing timestamp', { city: generated('x', limit + 1) }),
      parseCase('too long timestamp with invalid reason', { timestamp: generated('x', limit + 1), reason: 42 }),
      await readCase('valid json', { text: JSON.stringify({ timestamp: detection, reason: 'manual_save' }) }),
      await readCase('e2e sentry fixture', {
        text: JSON.stringify({
          timestamp: '2026-08-20T20:35:30',
          reason: 'sentry_aware_object_detection',
          city: 'SYNTHETIC_PRIVATE_CITY',
          street: 'SYNTHETIC_PRIVATE_STREET',
          est_lat: 'SYNTHETIC_PRIVATE_LAT',
          est_lon: 'SYNTHETIC_PRIVATE_LON',
        }, null, 2),
      }),
      await readCase('byte order mark', { text: `\uFEFF${JSON.stringify({ timestamp: detection })}` }),
      await readCase('two byte order marks', { text: `\uFEFF\uFEFF${JSON.stringify({ timestamp: detection })}` }),
      await readCase('duplicate keys last wins', { text: `{"timestamp": "first", "timestamp": "${detection}"}` }),
      await readCase('surrounding whitespace', { text: ` \n\t{"timestamp": "${detection}"}\r\n ` }),
      await readCase('escaped unicode', { text: '{"timestamp": "2026", "city": "\\u6e0b\\u8c37"}' }),
      await readCase('invalid json', { text: '{"timestamp": ' }),
      await readCase('trailing comma', { text: `{"timestamp": "${detection}",}` }),
      await readCase('empty file', { text: '' }),
      await readCase('json null', { text: 'null' }),
      await readCase('json array', { text: '[]' }),
      await readCase('invalid UTF-8 inside a string', {
        bytes: [...new TextEncoder().encode('{"timestamp": "a'), 0xe2, 0x82, ...new TextEncoder().encode('b", "city": "'), 0xf0, 0x9f, 0x98, 0xff, ...new TextEncoder().encode('"}')],
      }),
      await readCase('invalid UTF-8 outside strings', { bytes: [0xff, ...new TextEncoder().encode(`{"timestamp": "${detection}"}`)] }),
      await readCase('field over the limit', { text: JSON.stringify({ timestamp: detection, city: 'x'.repeat(limit + 1) }) }),
      await readCase('declared size over one MiB', { text: JSON.stringify({ timestamp: detection }), declaredSize: MAX_EVENT_METADATA_SIZE + 1 }),
      await readCase('declared size at one MiB', { text: JSON.stringify({ timestamp: detection }), declaredSize: MAX_EVENT_METADATA_SIZE }),
      await readCase('actual bytes over one MiB', { text: generated(' ', MAX_EVENT_METADATA_SIZE + 1), declaredSize: 1 }),
      await readCase('actual bytes at one MiB', { text: generated(' ', MAX_EVENT_METADATA_SIZE), declaredSize: 1 }),
    ],
  })
})
