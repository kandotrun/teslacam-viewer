import { describe, expect, it } from 'vitest'

import { loadCompositeVehicleImage } from './composite-asset'

describe('loadCompositeVehicleImage', () => {
  it('detaches the image listeners and clears the request when export is cancelled', async () => {
    const controller = new AbortController()
    const image = document.createElement('img')
    const sources: string[] = []
    Object.defineProperty(image, 'src', {
      set: (source: string) => {
        sources.push(source)
        if (source.length === 0) image.onerror?.(new Event('error'))
      },
    })

    const loading = loadCompositeVehicleImage({ createImage: () => image }, controller.signal)
    controller.abort()

    await expect(loading).rejects.toMatchObject({ code: 'aborted' })
    expect(sources).toEqual(['/vehicle-top.png', ''])
    expect(image.onload).toBeNull()
    expect(image.onerror).toBeNull()
  })
})
