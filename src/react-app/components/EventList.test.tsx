import { act, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { afterEach, vi } from 'vitest'

import type { LocalFileRef, TeslaCamEvent } from '../../teslacam/types'
import { EventList } from './EventList'

const events: readonly TeslaCamEvent[] = [
  {
    id: 'sentry-newer',
    category: 'sentry',
    timestamp: '2026-08-20_20-45-00',
    segments: [
      {
        timestamp: '2026-08-20_20-45-00',
        cameras: { front: { name: 'front.mp4', relativePath: 'SentryClips/event/2026-08-20_20-45-00-front.mp4', size: 12, type: 'video/mp4', getFile: async () => new File(['clip'], 'front.mp4') } },
      },
    ],
  },
  {
    id: 'recent-older',
    category: 'recent',
    timestamp: '2026-08-19_08-30-00',
    segments: [
      {
        timestamp: '2026-08-19_08-30-00',
        cameras: { front: { name: 'front.mp4', relativePath: 'RecentClips/2026-08-19_08-30-00-front.mp4', size: 12, type: 'video/mp4', getFile: async () => new File(['clip'], 'front.mp4') } },
      },
    ],
  },
]

function fileRef(name: string): LocalFileRef {
  return {
    name,
    relativePath: `RecentClips/${name}`,
    size: 12,
    type: 'video/mp4',
    getFile: async () => new File(['clip'], name, { type: 'video/mp4' }),
  }
}

function firstEvent(): TeslaCamEvent {
  const event = events.at(0)
  if (event === undefined) throw new Error('event fixture is missing')
  return event
}

function pendingFileRequest(): {
  readonly promise: Promise<File>
  readonly resolve: (file: File) => void
  readonly reject: (reason?: unknown) => void
} {
  let resolveFile: (file: File) => void = () => undefined
  let rejectFile: (reason?: unknown) => void = () => undefined
  const promise = new Promise<File>((resolve, reject) => {
    resolveFile = resolve
    rejectFile = reject
  })
  return { promise, resolve: resolveFile, reject: rejectFile }
}

function EventListHarness() {
  const [selectedEventId, setSelectedEventId] = useState<string | null>(null)

  return (
    <>
      <EventList
        events={events}
        selectedEventId={selectedEventId}
        onSelectEvent={setSelectedEventId}
      />
      <output>{selectedEventId ?? '未選択'}</output>
    </>
  )
}

afterEach(() => {
  vi.restoreAllMocks()
})

describe('EventList', () => {
  it('exposes an event-list disclosure control', async () => {
    const user = userEvent.setup()
    render(<EventListHarness />)
    const disclosure = screen.getByRole('button', { name: 'イベント一覧を閉じる' })

    await user.click(disclosure)

    expect(disclosure).toHaveAccessibleName('イベント一覧を開く')
    expect(disclosure).toHaveAttribute('aria-expanded', 'false')
  })

  it('lists matching events newest first with their missing camera count', () => {
    render(
      <EventList
        events={events}
        selectedEventId={null}
        onSelectEvent={() => undefined}
      />,
    )

    const eventButtons = screen.getAllByRole('button', { name: /2026-08/ })

    expect(eventButtons).toHaveLength(2)
    expect(eventButtons[0]).toHaveTextContent('2026-08-20 20:45:00')
    expect(eventButtons[0]).toHaveTextContent('1/6 カメラ')
    expect(eventButtons[0]).toHaveTextContent('1分')
    expect(eventButtons[0]).toHaveTextContent('1セグメント')
  })

  it('states camera availability positively and names missing directions only for partial events', () => {
    render(
      <EventList
        events={events}
        selectedEventId={null}
        onSelectEvent={() => undefined}
      />,
    )

    const partialEvent = screen.getByRole('button', { name: /2026-08-20 20:45:00/ })

    expect(partialEvent).toHaveTextContent('1/6 カメラ')
    expect(partialEvent).toHaveTextContent('リア、左ピラー、左リピーター、右ピラー、右リピーターなし')
    expect(partialEvent).not.toHaveTextContent('台のカメラがありません')
  })

  it('renders the matching subset supplied by the workspace', () => {
    render(
      <EventList
        events={[firstEvent()]}
        selectedEventId={null}
        onSelectEvent={() => undefined}
      />,
    )

    expect(screen.getByText('2026-08-20 20:45:00')).toBeVisible()
    expect(screen.queryByText('2026-08-19 08:30:00')).not.toBeInTheDocument()
  })

  it('selects an event through the rendered event control', async () => {
    const user = userEvent.setup()
    render(<EventListHarness />)

    await user.click(screen.getByRole('button', { name: /2026-08-20 20:45:00/ }))

    expect(screen.getByRole('status')).toHaveTextContent('sentry-newer')
  })

  it('shows an explicit empty result for an empty category', () => {
    render(
      <EventList
        events={[]}
        selectedEventId={null}
        onSelectEvent={() => undefined}
      />,
    )

    expect(screen.getByText('この条件に一致するイベントはありません。')).toBeVisible()
  })

  it('reports cameras missing from a later partial segment', () => {
    const fullSegment = {
      timestamp: '2026-08-20_20-45-00',
      cameras: {
        front: fileRef('front.mp4'),
        back: fileRef('back.mp4'),
        left_pillar: fileRef('left_pillar.mp4'),
        left_repeater: fileRef('left_repeater.mp4'),
        right_pillar: fileRef('right_pillar.mp4'),
        right_repeater: fileRef('right_repeater.mp4'),
      },
    }
    const partialSegment = {
      timestamp: '2026-08-20_20-46-00',
      cameras: { front: fileRef('later-front.mp4') },
    }
    const multiSegmentEvent: TeslaCamEvent = {
      id: 'recent-multi-segment',
      category: 'recent',
      timestamp: '2026-08-20_20-45-00',
      segments: [fullSegment, partialSegment],
    }

    render(
      <EventList
        events={[multiSegmentEvent]}
        selectedEventId={null}
        onSelectEvent={() => undefined}
      />,
    )

    expect(screen.getByRole('button', { name: /2026-08-20 20:45:00/ })).toHaveTextContent(
      '1/6 カメラ',
    )
  })

  it('creates and revokes a mounted thumbnail URL', async () => {
    const createObjectURL = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:thumbnail')
    const revokeObjectURL = vi.spyOn(URL, 'revokeObjectURL')
    const eventWithThumbnail: TeslaCamEvent = {
      ...firstEvent(),
      thumbnailFile: fileRef('thumb.png'),
    }
    const view = render(
      <EventList
        events={[eventWithThumbnail]}
        selectedEventId={null}
        onSelectEvent={() => undefined}
      />,
    )

    await waitFor(() => {
      expect(createObjectURL).toHaveBeenCalledTimes(1)
    })
    view.unmount()

    expect(revokeObjectURL).toHaveBeenCalledWith('blob:thumbnail')
  })

  it('does not create a thumbnail URL after an unmounted file request resolves', async () => {
    const pendingFile = pendingFileRequest()
    const createObjectURL = vi.spyOn(URL, 'createObjectURL')
    const thumbnailFile: LocalFileRef = {
      ...fileRef('pending-thumb.png'),
      getFile: async () => pendingFile.promise,
    }
    const eventWithPendingThumbnail: TeslaCamEvent = {
      ...firstEvent(),
      thumbnailFile,
    }
    const view = render(
      <EventList
        events={[eventWithPendingThumbnail]}
        selectedEventId={null}
        onSelectEvent={() => undefined}
      />,
    )

    view.unmount()
    pendingFile.resolve(new File(['thumb'], 'pending-thumb.png', { type: 'image/png' }))
    await Promise.resolve()

    expect(createObjectURL).not.toHaveBeenCalled()
  })

  it('does not draw a fake thumbnail frame when file loading rejects', async () => {
    const pendingFile = pendingFileRequest()
    const getFile = vi.fn(async () => pendingFile.promise)
    const eventWithRejectedThumbnail: TeslaCamEvent = {
      ...firstEvent(),
      thumbnailFile: {
        ...fileRef('rejected-thumb.png'),
        getFile,
      },
    }

    render(
      <EventList
        events={[eventWithRejectedThumbnail]}
        selectedEventId={null}
        onSelectEvent={() => undefined}
      />,
    )

    await waitFor(() => expect(getFile).toHaveBeenCalledTimes(1))
    await act(async () => {
      pendingFile.reject(new Error('thumbnail unavailable'))
      await pendingFile.promise.catch(() => undefined)
    })

    expect(document.querySelector('.event-thumbnail')).not.toBeInTheDocument()
  })
})
