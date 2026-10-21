/**
 * Golden fixture files shared with the Mac app (`packages/contract/fixtures`).
 *
 * Every `*.test.ts` file in this directory builds its cases, computes the expected values by calling the
 * real web functions and hands them to `defineFixture`. With `CONTRACT_WRITE=1` the fixture file is
 * (re)written; otherwise the committed file must equal what the web functions produce now, so CI fails
 * when the web behaviour drifts from the contract the Mac app is tested against.
 */
import { existsSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

import { expect, it } from 'vitest'

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | readonly JsonValue[]
  | { readonly [key: string]: JsonValue }

export type Platform = 'web' | 'mac'

export type FixtureCase = {
  readonly name: string
  readonly fn: string
  readonly input: JsonValue
  readonly expected: JsonValue
  readonly platforms?: readonly Platform[]
  readonly note?: string
}

type FixtureDefinition = {
  /** File name without `.json`, e.g. `clip-filename`. */
  readonly file: string
  /** Short module name shared by both test suites. */
  readonly module: string
  /** Web sources that produced the expected values (repository-relative). */
  readonly source: readonly string[]
  /** Absolute tolerance for numbers that are not integers. */
  readonly floatTolerance?: number
  readonly cases: () => readonly FixtureCase[] | Promise<readonly FixtureCase[]>
}

export const contractName = 'teslacam-viewer'
export const schemaVersion = 1
export const defaultFloatTolerance = 1e-9

export const fixturesDirectory = resolve(
  dirname(fileURLToPath(import.meta.url)),
  '../../../../packages/contract/fixtures',
)

// Keys that are written first, in this order, so the files read naturally. Every other key is sorted,
// which keeps the text stable when a web function builds its result objects in a different order.
const leadingKeys = [
  'contract',
  'schemaVersion',
  'module',
  'source',
  'floatTolerance',
  'cases',
  'name',
  'fn',
  'platforms',
  'note',
  'input',
  'expected',
] as const

function keyRank(key: string): number {
  const index = leadingKeys.findIndex((leadingKey) => leadingKey === key)
  return index < 0 ? leadingKeys.length : index
}

function compareKeys(left: string, right: string): number {
  const rankDifference = keyRank(left) - keyRank(right)
  if (rankDifference !== 0) return rankDifference
  if (left === right) return 0
  return left < right ? -1 : 1
}

function isJsonArray(value: JsonValue): value is readonly JsonValue[] {
  return Array.isArray(value)
}

function canonical(value: JsonValue, indent: string): string {
  if (value === null || typeof value === 'boolean' || typeof value === 'string') {
    return JSON.stringify(value)
  }
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new Error('fixtures encode non-finite numbers as strings')
    return JSON.stringify(Object.is(value, -0) ? 0 : value)
  }
  const inner = `${indent}  `
  if (isJsonArray(value)) {
    if (value.length === 0) return '[]'
    return `[\n${value.map((item) => `${inner}${canonical(item, inner)}`).join(',\n')}\n${indent}]`
  }
  const keys = Object.keys(value).sort(compareKeys)
  if (keys.length === 0) return '{}'
  const entries = keys.map((key) => {
    const item = value[key]
    if (item === undefined) throw new Error(`undefined value for key ${key}`)
    return `${inner}${JSON.stringify(key)}: ${canonical(item, inner)}`
  })
  return `{\n${entries.join(',\n')}\n${indent}}`
}

/** Serializes a JSON value with stable key order, 2-space indent and a trailing newline. */
export function serializeFixture(value: JsonValue): string {
  return `${canonical(value, '')}\n`
}

function caseJson(fixtureCase: FixtureCase): JsonValue {
  return {
    name: fixtureCase.name,
    fn: fixtureCase.fn,
    ...(fixtureCase.platforms === undefined ? {} : { platforms: fixtureCase.platforms }),
    ...(fixtureCase.note === undefined ? {} : { note: fixtureCase.note }),
    input: fixtureCase.input,
    expected: fixtureCase.expected,
  }
}

function assertUniqueNames(cases: readonly FixtureCase[]): void {
  const seen = new Set<string>()
  for (const fixtureCase of cases) {
    if (seen.has(fixtureCase.name)) throw new Error(`duplicate case name: ${fixtureCase.name}`)
    seen.add(fixtureCase.name)
  }
}

function committedCases(text: string): readonly JsonValue[] {
  const parsed: unknown = JSON.parse(text)
  if (typeof parsed !== 'object' || parsed === null || !('cases' in parsed)) return []
  const cases: unknown = parsed.cases
  return Array.isArray(cases) ? cases : []
}

export function defineFixture(definition: FixtureDefinition): void {
  it(`${definition.file}.json matches the web implementation`, async () => {
    const cases = await definition.cases()
    assertUniqueNames(cases)
    const generated = serializeFixture({
      contract: contractName,
      schemaVersion,
      module: definition.module,
      source: definition.source,
      ...(definition.floatTolerance === undefined ? {} : { floatTolerance: definition.floatTolerance }),
      cases: cases.map(caseJson),
    })
    const path = resolve(fixturesDirectory, `${definition.file}.json`)

    if (process.env['CONTRACT_WRITE'] === '1') {
      mkdirSync(fixturesDirectory, { recursive: true })
      writeFileSync(path, generated)
      return
    }

    expect(existsSync(path), `${path} is missing; run npm run contract:generate`).toBe(true)
    const committed = readFileSync(path, 'utf8')
    const expectedCases = committedCases(generated)
    const actualCases = committedCases(committed)
    for (const [index, expectedCase] of expectedCases.entries()) {
      expect(actualCases[index], `case #${index} of ${definition.file}.json`).toEqual(expectedCase)
    }
    expect(committed).toBe(generated)
  })
}
