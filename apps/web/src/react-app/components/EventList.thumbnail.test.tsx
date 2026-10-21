import { act, render, waitFor } from '@testing-library/react'
import { afterEach, vi } from 'vitest'

import type { LocalFileRef, TeslaCamEvent } from '../../teslacam/types'
import { EventList } from './EventList'
import { fileRef, firstEvent, pendingFileRequest } from './EventList.test-fixtures'

afterEach(() => vi.restoreAllMocks())

describe('EventList thumbnails', () => {
  it('creates and revokes a mounted thumbnail URL', async () => {
    const createObjectURL = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:thumbnail')
    const revokeObjectURL = vi.spyOn(URL, 'revokeObjectURL')
    const eventWithThumbnail: TeslaCamEvent = { ...firstEvent(), thumbnailFile: fileRef('thumb.png') }
    const view = render(<EventList events={[eventWithThumbnail]} selectedEventId={null} onSelectEvent={() => undefined} />)
    await waitFor(() => expect(createObjectURL).toHaveBeenCalledTimes(1))
    view.unmount()
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:thumbnail')
  })

  it('does not create a thumbnail URL after an unmounted file request resolves', async () => {
    const pendingFile = pendingFileRequest()
    const createObjectURL = vi.spyOn(URL, 'createObjectURL')
    const thumbnailFile: LocalFileRef = { ...fileRef('pending-thumb.png'), getFile: async () => pendingFile.promise }
    const eventWithPendingThumbnail: TeslaCamEvent = { ...firstEvent(), thumbnailFile }
    const view = render(<EventList events={[eventWithPendingThumbnail]} selectedEventId={null} onSelectEvent={() => undefined} />)
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
      thumbnailFile: { ...fileRef('rejected-thumb.png'), getFile },
    }
    render(<EventList events={[eventWithRejectedThumbnail]} selectedEventId={null} onSelectEvent={() => undefined} />)
    await waitFor(() => expect(getFile).toHaveBeenCalledTimes(1))
    await act(async () => {
      pendingFile.reject(new Error('thumbnail unavailable'))
      await pendingFile.promise.catch(() => undefined)
    })
    expect(document.querySelector('.event-thumbnail')).not.toBeInTheDocument()
  })
})
