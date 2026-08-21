import { parseClipFileName } from './filename'

describe('parseClipFileName', () => {
  it.each([
    ['front', 'front'],
    ['back', 'back'],
    ['left pillar', 'left_pillar'],
    ['left repeater', 'left_repeater'],
    ['right pillar', 'right_pillar'],
    ['right repeater', 'right_repeater'],
  ])('returns the %s camera from a valid Tesla clip name', (_, camera) => {
    const result = parseClipFileName(`2026-08-21_14-30-45-${camera}.mp4`)

    expect(result).toEqual({
      timestamp: '2026-08-21_14-30-45',
      camera,
    })
  })

  it('returns null for helpers and unknown camera suffixes', () => {
    expect(parseClipFileName('event.mp4')).toBeNull()
    expect(parseClipFileName('thumb.png')).toBeNull()
    expect(parseClipFileName('2026-08-21_14-30-45-cabin.mp4')).toBeNull()
  })

  it('returns null for a filename without a valid local timestamp', () => {
    expect(parseClipFileName('2026-02-29_14-30-45-front.mp4')).toBeNull()
    expect(parseClipFileName('2026-08-21_24-30-45-front.mp4')).toBeNull()
    expect(parseClipFileName('not-a-timestamp-front.mp4')).toBeNull()
  })
})
