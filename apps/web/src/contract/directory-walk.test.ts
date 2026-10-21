// @vitest-environment node
import { afterEach, describe } from 'vitest'

import { pickTeslaCamDirectory } from '../teslacam/directory-picker'
import { MAX_DIRECTORY_DEPTH, MAX_SCAN_ENTRIES, ScanLimitError } from '../teslacam/scan-limits'
import type { ScanProgress } from '../teslacam/types'
import { defineFixture, type FixtureCase, type JsonValue } from './fixture-file'

/**
 * Directory tree given in enumeration order. `generate` appends `count` files named by `namePattern`
 * (`{i}` = 0-based index). `symlink` entries exist only on the Mac, which counts them but neither
 * follows nor collects them; the web reference therefore walks them as hidden placeholder files.
 */
type WalkNode =
  | { readonly kind: 'file'; readonly name: string; readonly size: number }
  | {
      readonly kind: 'directory'
      readonly name: string
      readonly entries?: readonly WalkNode[]
      readonly generate?: { readonly count: number; readonly namePattern: string; readonly size?: number }
    }
  | { readonly kind: 'symlink'; readonly name: string }

type FakeHandle =
  | { readonly kind: 'file'; readonly name: string; readonly getFile: () => Promise<File> }
  | { readonly kind: 'directory'; readonly name: string; readonly values: () => AsyncIterable<FakeHandle> }

function file(name: string, size = 1): WalkNode {
  return { kind: 'file', name, size }
}

function directory(name: string, entries: readonly WalkNode[]): WalkNode {
  return { kind: 'directory', name, entries }
}

function fileHandle(name: string, size: number, reportedName = name): FakeHandle {
  return { kind: 'file', name, getFile: async () => new File([new Uint8Array(size)], reportedName) }
}

function* childNodes(node: Extract<WalkNode, { kind: 'directory' }>): Generator<WalkNode> {
  yield* node.entries ?? []
  const generate = node.generate
  if (generate === undefined) return
  for (let index = 0; index < generate.count; index += 1) {
    yield file(generate.namePattern.replaceAll('{i}', String(index)), generate.size ?? 1)
  }
}

function toHandle(node: WalkNode): FakeHandle {
  switch (node.kind) {
    case 'file':
      return fileHandle(node.name, node.size)
    case 'symlink':
      return fileHandle(`.mac-symlink-${node.name}`, 0)
    case 'directory':
      return {
        kind: 'directory',
        name: node.name,
        values: async function* () {
          for (const child of childNodes(node)) yield toHandle(child)
        },
      }
  }
}

function nodeJson(node: WalkNode): JsonValue {
  switch (node.kind) {
    case 'file':
      return { kind: 'file', name: node.name, size: node.size }
    case 'symlink':
      return { kind: 'symlink', name: node.name }
    case 'directory':
      return {
        kind: 'directory',
        name: node.name,
        ...(node.entries === undefined ? {} : { entries: node.entries.map(nodeJson) }),
        ...(node.generate === undefined
          ? {}
          : {
              generate: {
                count: node.generate.count,
                namePattern: node.generate.namePattern,
                ...(node.generate.size === undefined ? {} : { size: node.generate.size }),
              },
            }),
      }
  }
}

function containsSymlink(node: WalkNode): boolean {
  return node.kind === 'symlink' || (node.kind === 'directory' && (node.entries ?? []).some(containsSymlink))
}

async function walkCase(name: string, root: Extract<WalkNode, { kind: 'directory' }>): Promise<FixtureCase> {
  Object.defineProperty(globalThis, 'showDirectoryPicker', {
    configurable: true,
    value: async () => toHandle(root),
  })
  const progress: ScanProgress[] = []
  let expected: JsonValue
  try {
    const result = await pickTeslaCamDirectory((update) => {
      progress.push(update)
    })
    if (result.status !== 'selected') throw new Error(`unexpected status ${result.status}`)
    expected = {
      status: 'selected',
      folderName: result.folderName,
      files: result.files.map((entry) => ({ name: entry.name, relativePath: entry.relativePath, size: entry.size })),
      progress: progress.map((update) => ({ processed: update.processed, total: update.total })),
    }
  } catch (error) {
    if (!(error instanceof ScanLimitError)) throw error
    expected = { status: 'scan-limit', kind: error.kind }
  }
  return {
    name,
    fn: 'walkDirectory',
    ...(containsSymlink(root) ? { platforms: ['mac'] } : {}),
    input: { root: nodeJson(root) },
    expected,
  }
}

function chain(depth: number): Extract<WalkNode, { kind: 'directory' }> {
  let node: Extract<WalkNode, { kind: 'directory' }> = { kind: 'directory', name: `depth-${depth}`, entries: [] }
  for (let level = depth - 1; level >= 0; level -= 1) {
    node = { kind: 'directory', name: `depth-${level}`, entries: [node] }
  }
  return node
}

function chainWithFile(depth: number): Extract<WalkNode, { kind: 'directory' }> {
  let node: Extract<WalkNode, { kind: 'directory' }> = {
    kind: 'directory',
    name: `depth-${depth}`,
    entries: [file('2026-08-21_14-30-45-front.mp4')],
  }
  for (let level = depth - 1; level >= 0; level -= 1) {
    node = { kind: 'directory', name: `depth-${level}`, entries: [node] }
  }
  return node
}

afterEach(() => {
  Reflect.deleteProperty(globalThis, 'showDirectoryPicker')
})

describe('contract: directory-walk', () => {
  defineFixture({
    file: 'directory-walk',
    module: 'directory-walk',
    source: ['apps/web/src/teslacam/directory-picker.ts', 'apps/web/src/teslacam/scan-limits.ts'],
    cases: async () => [
      await walkCase('nested TeslaCam root with hidden entries', {
        kind: 'directory',
        name: 'TeslaCam',
        entries: [
          directory('SavedClips', [
            directory('saved-a', [file('2026-08-21_14-30-45-front.mp4', 4), file('.DS_Store', 6)]),
            directory('.ignored', [file('2026-08-21_14-30-45-back.mp4')]),
          ]),
          file('._2026-08-21_14-30-45-front.mp4', 2),
        ],
      }),
      ...await Promise.all(['RecentClips', 'SavedClips', 'SentryClips'].map((category) =>
        walkCase(`root named ${category}`, {
          kind: 'directory',
          name: category,
          entries: [file('2026-08-21_14-30-45-front.mp4')],
        }))),
      await walkCase('zero-byte file kept', {
        kind: 'directory',
        name: 'RecentClips',
        entries: [file('2026-08-21_14-30-45-front.mp4', 0)],
      }),
      await walkCase('empty root', { kind: 'directory', name: 'TeslaCam', entries: [] }),
      await walkCase('entries keep enumeration order', {
        kind: 'directory',
        name: 'TeslaCam',
        entries: [
          directory('SentryClips', [directory('b', [file('2', 2)]), directory('a', [file('1', 1)])]),
          file('z.txt', 3),
          directory('RecentClips', [file('y', 1), file('x', 1)]),
        ],
      }),
      await walkCase('names with spaces and Japanese', {
        kind: 'directory',
        name: 'TeslaCam 2026',
        entries: [directory('保存 フォルダ', [file('メモ.txt', 9)])],
      }),
      await walkCase('250 entries report the boundary twice', {
        kind: 'directory',
        name: 'RecentClips',
        generate: { count: 250, namePattern: '.entry-{i}' },
      }),
      await walkCase('251 entries', {
        kind: 'directory',
        name: 'RecentClips',
        generate: { count: 251, namePattern: '.entry-{i}' },
      }),
      await walkCase('visible generated files', {
        kind: 'directory',
        name: 'RecentClips',
        entries: [file('first.mp4', 7)],
        generate: { count: 3, namePattern: '2026-08-21_14-3{i}-45-front.mp4', size: 5 },
      }),
      await walkCase('hidden and directory entries are counted', {
        kind: 'directory',
        name: 'TeslaCam',
        entries: [directory('.hidden', [file('never-counted')]), directory('Noise', [])],
        generate: { count: 248, namePattern: '.hidden-{i}' },
      }),
      await walkCase('e2e full tree', {
        kind: 'directory',
        name: 'TeslaCam',
        entries: [
          { kind: 'directory', name: 'Noise', generate: { count: 253, namePattern: 'synthetic-{i}.txt' } },
          directory('RecentClips', [
            'front', 'back', 'left_pillar', 'left_repeater', 'right_pillar', 'right_repeater',
          ].map((camera) => file(`2026-08-20_20-45-00-${camera}.mp4`, 2048))),
          directory('SavedClips', [directory('event-saved', [file('2026-08-20_20-40-00-front.mp4', 2048), file('event.mp4', 2048)])]),
          directory('SentryClips', [directory('event-sentry', [file('2026-08-20_20-35-00-front.mp4', 2048), file('event.json', 224)])]),
        ],
      }),
      await walkCase(`depth ${MAX_DIRECTORY_DEPTH} is allowed`, chainWithFile(MAX_DIRECTORY_DEPTH)),
      await walkCase(`depth ${MAX_DIRECTORY_DEPTH + 1} fails`, chain(MAX_DIRECTORY_DEPTH + 1)),
      await walkCase(`entry count ${MAX_SCAN_ENTRIES} is allowed`, {
        kind: 'directory',
        name: 'Noise',
        generate: { count: MAX_SCAN_ENTRIES, namePattern: '.n{i}' },
      }),
      await walkCase(`entry count ${MAX_SCAN_ENTRIES + 1} fails`, {
        kind: 'directory',
        name: 'Noise',
        generate: { count: MAX_SCAN_ENTRIES + 1, namePattern: '.n{i}' },
      }),
      await walkCase('symbolic links are counted but not followed', {
        kind: 'directory',
        name: 'TeslaCam',
        entries: [
          { kind: 'symlink', name: 'RecentClips' },
          directory('SavedClips', [directory('a', [{ kind: 'symlink', name: '2026-08-21_14-30-45-front.mp4' }, file('2026-08-21_14-30-45-back.mp4')])]),
        ],
      }),
    ],
  })
})
