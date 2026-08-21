import { afterEach, vi } from 'vitest'

import { ObjectUrlRegistry } from './object-url-registry'

function videoFile(name: string): File {
  return new File(['video'], name, { type: 'video/mp4' })
}

afterEach(() => {
  vi.restoreAllMocks()
})

describe('ObjectUrlRegistry', () => {
  it('reuses the same URL until each acquisition is released', () => {
    const createObjectURL = vi.spyOn(URL, 'createObjectURL').mockReturnValue('blob:one')
    const revokeObjectURL = vi.spyOn(URL, 'revokeObjectURL')
    const registry = new ObjectUrlRegistry()
    const file = videoFile('front.mp4')

    const firstUrl = registry.acquire(file)
    const secondUrl = registry.acquire(file)

    expect(firstUrl).toBe('blob:one')
    expect(secondUrl).toBe(firstUrl)
    expect(createObjectURL).toHaveBeenCalledTimes(1)

    registry.release(file)

    expect(revokeObjectURL).not.toHaveBeenCalled()

    registry.release(file)

    expect(revokeObjectURL).toHaveBeenCalledTimes(1)
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:one')
  })

  it('leaves unknown files without creating or revoking URLs', () => {
    const createObjectURL = vi.spyOn(URL, 'createObjectURL')
    const revokeObjectURL = vi.spyOn(URL, 'revokeObjectURL')
    const registry = new ObjectUrlRegistry()

    registry.release(videoFile('unknown.mp4'))

    expect(createObjectURL).not.toHaveBeenCalled()
    expect(revokeObjectURL).not.toHaveBeenCalled()
  })

  it('revokes every live URL once, resets state, and can acquire again after disposal', () => {
    const createObjectURL = vi
      .spyOn(URL, 'createObjectURL')
      .mockReturnValueOnce('blob:first')
      .mockReturnValueOnce('blob:second')
      .mockReturnValueOnce('blob:third')
    const revokeObjectURL = vi.spyOn(URL, 'revokeObjectURL')
    const registry = new ObjectUrlRegistry()
    const front = videoFile('front.mp4')
    const back = videoFile('back.mp4')

    registry.acquire(front)
    registry.acquire(front)
    registry.acquire(back)

    registry.dispose()
    registry.dispose()

    expect(createObjectURL).toHaveBeenCalledTimes(2)
    expect(revokeObjectURL).toHaveBeenCalledTimes(2)
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:first')
    expect(revokeObjectURL).toHaveBeenCalledWith('blob:second')

    expect(registry.acquire(front)).toBe('blob:third')

    registry.release(front)

    expect(revokeObjectURL).toHaveBeenCalledTimes(3)
    expect(revokeObjectURL).toHaveBeenLastCalledWith('blob:third')
  })
})
