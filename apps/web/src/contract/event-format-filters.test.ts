// @vitest-environment node
import { describe } from 'vitest'

import {
  categoryLabel,
  eventClock,
  eventDate,
  eventMatchesFilters,
  eventTime,
  orderedMatchingEvents,
  type EventFilters,
} from '../react-app/event-filters'
import type { ClipCategory } from '../teslacam/types'
import { eventFixture, eventFixtureJson, toTeslaCamEvent, type EventFixture } from './adapters'
import { defineFixture, type FixtureCase, type JsonValue } from './fixture-file'

const categories: readonly ClipCategory[] = ['recent', 'saved', 'sentry']

function filtersJson(filters: EventFilters): JsonValue {
  return { category: filters.category, dateKey: filters.dateKey, query: filters.query }
}

function filters(partial: Partial<EventFilters>): EventFilters {
  return { category: 'all', dateKey: null, query: '', ...partial }
}

function textCases(timestamp: string): FixtureCase[] {
  return [
    { name: `eventDate ${timestamp}`, fn: 'eventDate', input: { timestamp }, expected: eventDate(timestamp) },
    { name: `eventClock ${timestamp}`, fn: 'eventClock', input: { timestamp }, expected: eventClock(timestamp) },
    { name: `eventTime ${timestamp}`, fn: 'eventTime', input: { timestamp }, expected: eventTime(timestamp) },
  ]
}

function matchCase(name: string, event: EventFixture, value: EventFilters): FixtureCase {
  return {
    name: `matches ${name}`,
    fn: 'eventMatchesFilters',
    input: { event: eventFixtureJson(event), filters: filtersJson(value) },
    expected: eventMatchesFilters(toTeslaCamEvent(event), value),
  }
}

function orderedCase(name: string, events: readonly EventFixture[], value: EventFilters): FixtureCase {
  return {
    name: `ordered ${name}`,
    fn: 'orderedMatchingEvents',
    input: { events: events.map(eventFixtureJson), filters: filtersJson(value) },
    expected: orderedMatchingEvents(events.map(toTeslaCamEvent), value).map((event) => event.id),
  }
}

const sentry = eventFixture('sentry-2026-08-20_20-45-00-1', 'sentry', '2026-08-20_20-45-00')
const saved = eventFixture('saved-2026-08-20_20-40-00-1', 'saved', '2026-08-20_20-40-00')
const recent = eventFixture('recent-2026-08-21_08-00-00-1', 'recent', '2026-08-21_08-00-00')

const queries = [
  '',
  '   ',
  '20:45',
  '20-45',
  ' SENTRY ',
  'sentry',
  'Sentry',
  'セントリー',
  'セント',
  '2026-08-20 20',
  '2026-08-20 20:45:00 sentry セントリー',
  'saved',
  '保存済み',
  '最近',
  'recent',
  '\u3000sentry\u3000',
  '\uFEFFsentry',
  '\u00A0sentry\u00A0',
  '\u0085sentry',
  '\tsentry\n',
  '2026-08-20_20-45-00',
  'x',
]

describe('contract: event-format-filters', () => {
  defineFixture({
    file: 'event-format-filters',
    module: 'event-filters',
    source: ['apps/web/src/react-app/event-filters.ts'],
    cases: () => [
      ...['2026-08-20_20-45-00', '2026-01-01_00-00-00', '2028-02-29_23-59-59'].flatMap(textCases),
      ...categories.map((category): FixtureCase => ({
        name: `categoryLabel ${category}`,
        fn: 'categoryLabel',
        input: { category },
        expected: categoryLabel(category),
      })),
      ...queries.map((query) => matchCase(`query ${JSON.stringify(query)}`, sentry, filters({ query }))),
      matchCase('query on a saved event', saved, filters({ query: '保存' })),
      matchCase('query on a recent event', recent, filters({ query: '走行' })),
      matchCase('category all', sentry, filters({ category: 'all' })),
      matchCase('category sentry', sentry, filters({ category: 'sentry' })),
      matchCase('category saved rejects sentry', sentry, filters({ category: 'saved' })),
      matchCase('date prefix match', sentry, filters({ dateKey: '2026-08-20' })),
      matchCase('date prefix mismatch', sentry, filters({ dateKey: '2026-08-21' })),
      matchCase('date raw prefix', sentry, filters({ dateKey: '2026-08' })),
      matchCase('empty date key', sentry, filters({ dateKey: '' })),
      matchCase('all filters together', sentry, filters({ category: 'sentry', dateKey: '2026-08-20', query: '20:45' })),
      matchCase('category and query mismatch', sentry, filters({ category: 'sentry', query: 'saved' })),
      orderedCase('newest first', [saved, recent, sentry], filters({})),
      orderedCase('filtered by category', [saved, recent, sentry], filters({ category: 'saved' })),
      orderedCase('filtered by date', [saved, recent, sentry], filters({ dateKey: '2026-08-20' })),
      orderedCase('no match', [saved, recent, sentry], filters({ query: 'nothing' })),
      orderedCase('stable ties keep input order', [
        eventFixture('saved-b', 'saved', '2026-08-20_10-00-00'),
        eventFixture('sentry-a', 'sentry', '2026-08-20_10-00-00'),
        eventFixture('recent-c', 'recent', '2026-08-20_10-00-00'),
        eventFixture('saved-newer', 'saved', '2026-08-20_11-00-00'),
      ], filters({})),
      orderedCase('year boundary', [
        eventFixture('old', 'recent', '2026-12-31_23-59-59'),
        eventFixture('new', 'recent', '2027-01-01_00-00-00'),
      ], filters({})),
    ],
  })
})
