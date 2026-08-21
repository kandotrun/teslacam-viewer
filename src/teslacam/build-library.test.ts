import { afterEach, vi } from 'vitest'

import { createLocalFileRef } from '../test/teslacam-fixtures'
import { buildTeslaCamLibrary } from './build-library'
import type { ScanProgress } from './types'

describe('buildTeslaCamLibrary', () => {
  afterEach(() => {
    vi.useRealTimers()
  })

  it('groups saved camera clips by parent event and timestamp', async () => {
    const front = createLocalFileRef(
      'TeslaCam/SavedClips/saved-a/2026-08-21_14-30-45-front.mp4',
    )
    const back = createLocalFileRef(
      'TeslaCam/SavedClips/saved-a/2026-08-21_14-30-45-back.mp4',
    )
    const laterFront = createLocalFileRef(
      'TeslaCam/SavedClips/saved-a/2026-08-21_14-31-45-front.mp4',
    )
    const representative = createLocalFileRef(
      'TeslaCam/SavedClips/saved-a/event.mp4',
    )
    const thumbnail = createLocalFileRef(
      'TeslaCam/SavedClips/saved-a/thumb.png',
      { type: 'image/png' },
    )
    const metadata = createLocalFileRef(
      'TeslaCam/SavedClips/saved-a/event.json',
      {
        contents: JSON.stringify({
          timestamp: '2026-08-21_14-30-45',
          reason: 'manual_save',
          ignored: 'untrusted',
        }),
        type: 'application/json',
      },
    )
    const newerSaved = createLocalFileRef(
      'TeslaCam/SavedClips/saved-b/2026-08-21_15-30-45-left_pillar.mp4',
    )

    const library = await buildTeslaCamLibrary(
      [front, back, laterFront, representative, thumbnail, metadata, newerSaved],
      () => undefined,
    )
    const savedEvents = library.eventsByCategory.saved
    const firstSavedEvent = savedEvents.at(0)
    const secondSavedEvent = savedEvents.at(1)
    const firstSegment = secondSavedEvent?.segments.at(0)
    const secondSegment = secondSavedEvent?.segments.at(1)

    expect(savedEvents).toHaveLength(2)
    expect(firstSavedEvent?.timestamp).toBe('2026-08-21_15-30-45')
    expect(secondSavedEvent?.segments).toHaveLength(2)
    expect(firstSegment?.cameras.front).toBe(front)
    expect(firstSegment?.cameras.back).toBe(back)
    expect(secondSegment?.cameras.front).toBe(laterFront)
    expect(secondSavedEvent?.representativeFile).toBe(representative)
    expect(secondSavedEvent?.thumbnailFile).toBe(thumbnail)
    expect(secondSavedEvent?.metadata).toEqual({
      timestamp: '2026-08-21_14-30-45',
      reason: 'manual_save',
    })
  })

  it('retains all six cameras in one same-timestamp segment', async () => {
    const front = createLocalFileRef(
      'TeslaCam/SavedClips/saved-six/2026-08-21_16-30-45-front.mp4',
    )
    const back = createLocalFileRef(
      'TeslaCam/SavedClips/saved-six/2026-08-21_16-30-45-back.mp4',
    )
    const leftPillar = createLocalFileRef(
      'TeslaCam/SavedClips/saved-six/2026-08-21_16-30-45-left_pillar.mp4',
    )
    const leftRepeater = createLocalFileRef(
      'TeslaCam/SavedClips/saved-six/2026-08-21_16-30-45-left_repeater.mp4',
    )
    const rightPillar = createLocalFileRef(
      'TeslaCam/SavedClips/saved-six/2026-08-21_16-30-45-right_pillar.mp4',
    )
    const rightRepeater = createLocalFileRef(
      'TeslaCam/SavedClips/saved-six/2026-08-21_16-30-45-right_repeater.mp4',
    )

    const library = await buildTeslaCamLibrary(
      [front, back, leftPillar, leftRepeater, rightPillar, rightRepeater],
      () => undefined,
    )

    expect(library.eventsByCategory.saved[0]?.segments[0]?.cameras).toEqual({
      front,
      back,
      left_pillar: leftPillar,
      left_repeater: leftRepeater,
      right_pillar: rightPillar,
      right_repeater: rightRepeater,
    })
  })

  it('keeps sentry events separate by their parent directories', async () => {
    const first = createLocalFileRef(
      'TeslaCam/SentryClips/sentry-a/2026-08-21_14-30-45-right_repeater.mp4',
    )
    const second = createLocalFileRef(
      'TeslaCam/SentryClips/sentry-b/2026-08-21_14-30-45-right_repeater.mp4',
    )

    const library = await buildTeslaCamLibrary([first, second], () => undefined)

    expect(library.eventsByCategory.sentry).toHaveLength(2)
    expect(library.eventsByCategory.sentry[0]?.segments[0]?.cameras.right_repeater).toBe(
      first,
    )
    expect(library.eventsByCategory.sentry[1]?.segments[0]?.cameras.right_repeater).toBe(
      second,
    )
  })

  it('excludes empty and non-playable files while retaining sanitized counts', async () => {
    const emptyClip = createLocalFileRef(
      'TeslaCam/SavedClips/saved-a/2026-08-21_14-30-45-front.mp4',
      { size: 0 },
    )
    const unknownCamera = createLocalFileRef(
      'TeslaCam/SavedClips/saved-a/2026-08-21_14-30-45-cabin.mp4',
    )
    const unknownFile = createLocalFileRef(
      'TeslaCam/SavedClips/saved-a/unrecognized.bin',
      { type: 'application/octet-stream' },
    )
    const encryptedClip = createLocalFileRef(
      'TeslaCam/EncryptedClips/2026-08-21_14-30-45-front.mp4',
    )

    const library = await buildTeslaCamLibrary(
      [emptyClip, unknownCamera, unknownFile, encryptedClip],
      () => undefined,
    )

    expect(library.events).toEqual([])
    expect(library.warnings).toEqual([
      { kind: 'empty-media-file', count: 1 },
      { kind: 'unknown-file', count: 2 },
      { kind: 'encrypted-file', count: 1 },
    ])
    expect(JSON.stringify(library.warnings)).not.toContain('TeslaCam')
    expect(JSON.stringify(library.warnings)).not.toContain('unrecognized')
  })

  it('joins recent segments no more than ninety seconds apart across midnight', async () => {
    const first = createLocalFileRef(
      'TeslaCam/RecentClips/2026-08-21_23-59-30-front.mp4',
    )
    const joined = createLocalFileRef(
      'TeslaCam/RecentClips/2026-08-22_00-00-50-back.mp4',
    )
    const split = createLocalFileRef(
      'TeslaCam/RecentClips/2026-08-22_00-02-21-front.mp4',
    )

    const library = await buildTeslaCamLibrary([first, joined, split], () => undefined)
    const recentEvents = library.eventsByCategory.recent
    const olderSession = recentEvents.at(1)

    expect(recentEvents).toHaveLength(2)
    expect(recentEvents[0]?.timestamp).toBe('2026-08-22_00-02-21')
    expect(olderSession?.timestamp).toBe('2026-08-21_23-59-30')
    expect(olderSession?.segments).toHaveLength(2)
    expect(olderSession?.segments[1]?.cameras.back).toBe(joined)
  })

  it('reports every 250 entries and yields before completion', async () => {
    vi.useFakeTimers()
    const files = Array.from({ length: 251 }, (_, index) =>
      createLocalFileRef(`TeslaCam/RecentClips/unknown-${index}.txt`, {
        type: 'text/plain',
      }),
    )
    const progress: ScanProgress[] = []

    const libraryPromise = buildTeslaCamLibrary(files, (update) => {
      progress.push(update)
    })

    await Promise.resolve()

    expect(progress).toEqual([{ processed: 250, total: 251 }])

    await vi.advanceTimersByTimeAsync(0)
    const library = await libraryPromise

    expect(progress).toEqual([
      { processed: 250, total: 251 },
      { processed: 251, total: 251 },
    ])
    expect(library.warnings).toEqual([{ kind: 'unknown-file', count: 251 }])
  })
})
