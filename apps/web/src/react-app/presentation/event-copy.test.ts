import type { CameraId, ClipCategory, TeslaCamEvent } from '../../teslacam/types'
import { cameraAvailability } from '../event-summary'
import {
  cameraCountText,
  coordinatesText,
  dayTitle,
  detectionFact,
  detectionSentence,
  locationText,
  missingCamerasText,
  playbackStartText,
  scanProgressCopy,
  scrubberValueText,
  sentryDetection,
  startClock,
  startsAtPreRoll,
  transportTimeLabel,
  warningText,
} from './event-copy'

function event(
  category: ClipCategory,
  segments: readonly string[],
  options: { readonly detection?: string; readonly reason?: string; readonly cameras?: readonly CameraId[] } = {},
): TeslaCamEvent {
  const cameras = options.cameras ?? ['front', 'back', 'left_pillar', 'left_repeater', 'right_pillar', 'right_repeater']
  const file = { name: 'clip.mp4', relativePath: 'clip.mp4', size: 1, type: 'video/mp4', getFile: async () => new File([], 'clip.mp4') }
  return {
    id: `${category}-1`,
    category,
    timestamp: segments[0] ?? '2026-08-20_20-35-00',
    segments: segments.map((timestamp) => ({ timestamp, cameras: Object.fromEntries(cameras.map((camera) => [camera, file])) })),
    ...(options.detection === undefined
      ? {}
      : { metadata: { timestamp: options.detection, ...(options.reason === undefined ? {} : { reason: options.reason }) } }),
  }
}

describe('event copy', () => {
  it('states healthy camera counts positively and names missing directions', () => {
    const full = cameraAvailability(event('saved', ['2026-08-20_20-40-00']))
    const partial = cameraAvailability(event('sentry', ['2026-08-20_20-35-00'], { cameras: ['front', 'back', 'left_pillar', 'left_repeater', 'right_pillar'] }))

    expect(cameraCountText(full)).toBe('6カメラ')
    expect(missingCamerasText(full)).toBeNull()
    expect(cameraCountText(partial)).toBe('5カメラ')
    expect(missingCamerasText(partial)).toBe('右リピーターの記録がありません')
  })

  it('reads the start clock as hours and minutes', () => {
    expect(startClock('2026-08-20_20-45-00')).toBe('20:45')
  })

  it('describes a Sentry detection with its reason in words', () => {
    const sentry = event('sentry', ['2026-08-20_20-35-00'], {
      detection: '2026-08-20T20:35:30',
      reason: 'sentry_aware_object_detection',
    })
    const detection = sentryDetection(sentry)

    expect(detection).toEqual({ seconds: 30, timeLabel: '20:35:30', reasonWords: '物体の接近を' })
    if (detection === null) throw new Error('detection is missing')
    expect(detectionFact(detection)).toBe('20:35:30に検知')
    expect(detectionSentence(detection)).toBe('物体の接近を20:35:30に検知')
  })

  it('shows no detection for saved or recent events even with an in-range timestamp', () => {
    expect(sentryDetection(event('saved', ['2026-08-20_20-40-00'], { detection: '2026-08-20T20:40:30' }))).toBeNull()
    expect(sentryDetection(event('recent', ['2026-08-20_20-45-00'], { detection: '2026-08-20T20:45:30' }))).toBeNull()
  })

  it('labels the pre-roll only when the detection is at least 15 s into the event', () => {
    const late = event('sentry', ['2026-08-24_12-00-00', '2026-08-24_12-01-00'], { detection: '2026-08-24T12:01:20' })
    const early = event('sentry', ['2026-08-24_12-00-00'], { detection: '2026-08-24T12:00:10' })
    const exactly = event('sentry', ['2026-08-24_12-00-00'], { detection: '2026-08-24T12:00:15' })

    expect(startsAtPreRoll(late)).toBe(true)
    expect(playbackStartText(late)).toBe('12:01:05（検知の15秒前）')
    expect(startsAtPreRoll(early)).toBe(false)
    expect(playbackStartText(early)).toBe('12:00:00（記録の最初から）')
    expect(startsAtPreRoll(exactly)).toBe(true)
    expect(playbackStartText(exactly)).toBe('12:00:00（検知の15秒前）')
    expect(playbackStartText(event('saved', ['2026-08-24_12-00-00'], { detection: '2026-08-24T12:00:30' }))).toBeNull()
  })

  it('joins trimmed location parts and requires both coordinates', () => {
    expect(locationText({ timestamp: 't', city: ' 渋谷区 ', street: '神南一丁目' })).toBe('渋谷区 神南一丁目')
    expect(locationText({ timestamp: 't', street: 'Sample street' })).toBe('Sample street')
    expect(locationText(undefined)).toBeNull()
    expect(coordinatesText({ timestamp: 't', est_lat: '35.0000', est_lon: '139.0000' })).toBe('35.0000, 139.0000')
    expect(coordinatesText({ timestamp: 't', est_lat: '35.0000' })).toBeNull()
  })

  it('titles a day with its date and weekday', () => {
    expect(dayTitle('2026-08-20')).toEqual({ date: '8月20日', weekday: '木曜日' })
    expect(dayTitle('2026-08-24')).toEqual({ date: '8月24日', weekday: '月曜日' })
    expect(dayTitle('2026-02-30')).toBeNull()
  })

  it('formats the transport clock and the slider value text', () => {
    expect(transportTimeLabel(65.4)).toBe('1:05')
    expect(transportTimeLabel(300)).toBe('5:00')
    expect(scrubberValueText(15.2, 60)).toBe('0分15秒 / 1分')
    expect(scrubberValueText(75, 120)).toBe('1分15秒 / 2分')
  })

  it('counts scanned entries with digit grouping', () => {
    expect(scanProgressCopy({ processed: 0, total: 0 })).toEqual({ count: null, label: 'フォルダを読み込んでいます。' })
    expect(scanProgressCopy({ processed: 1250, total: 1250 })).toEqual({ count: '1,250', label: '件を確認しました' })
    expect(scanProgressCopy({ processed: 250, total: 1250 })).toEqual({ count: '250 / 1,250', label: '件を確認しました' })
  })

  it('writes one sentence per scan warning', () => {
    expect(warningText({ kind: 'encrypted-file', count: 3 })).toBe('暗号化されたファイル 3 件は読み込みませんでした。')
    expect(warningText({ kind: 'unknown-file', count: 1234 })).toBe('対応外のファイル 1234 件を除外しました。')
  })
})
