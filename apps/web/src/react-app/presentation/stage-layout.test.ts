import { cameraIds } from '../../teslacam/types'
import { effectiveViewMode, stageMode, viewModesFor, windowArea } from './stage-layout'

describe('effectiveViewMode', () => {
  it('switches to the representative video only when every camera failed and event.mp4 exists', () => {
    expect(effectiveViewMode('spatial', true, true)).toBe('representative')
    expect(effectiveViewMode('grid', true, false)).toBe('grid')
    expect(effectiveViewMode('spatial', false, true)).toBe('spatial')
    expect(effectiveViewMode('representative', false, false)).toBe('spatial')
  })
})

describe('stageMode', () => {
  it('expands a camera on desktop and shows one camera on phones', () => {
    expect(stageMode('spatial', 'back', 'desktop')).toBe('expanded')
    expect(stageMode('grid', null, 'tablet')).toBe('grid')
    expect(stageMode('spatial', null, 'phone')).toBe('single')
    expect(stageMode('representative', 'back', 'desktop')).toBe('representative')
  })
})

describe('windowArea', () => {
  it('keeps windows in their own areas for spatial and grid views', () => {
    expect(cameraIds.map((cameraId) => windowArea('spatial', cameraId, 'front'))).toEqual([...cameraIds])
  })

  it('moves the expanded camera to the main area and the rest to the strip in canonical order', () => {
    expect(cameraIds.map((cameraId) => windowArea('expanded', cameraId, 'left_pillar'))).toEqual([
      's1',
      's2',
      'main',
      's3',
      's4',
      's5',
    ])
  })

  it('puts all six cameras in the strip beside the representative video', () => {
    expect(cameraIds.map((cameraId) => windowArea('representative', cameraId, 'front'))).toEqual(['s1', 's2', 's3', 's4', 's5', 's6'])
  })

  it('shows only the active camera on a phone', () => {
    expect(cameraIds.map((cameraId) => windowArea('single', cameraId, 'back'))).toEqual(['hidden', 'main', 'hidden', 'hidden', 'hidden', 'hidden'])
  })
})

describe('viewModesFor', () => {
  it('offers the representative view only with event.mp4 and no grid on phones', () => {
    expect(viewModesFor(false, 'desktop')).toEqual(['spatial', 'grid'])
    expect(viewModesFor(true, 'tablet')).toEqual(['spatial', 'grid', 'representative'])
    expect(viewModesFor(true, 'phone')).toEqual(['spatial', 'representative'])
  })
})
