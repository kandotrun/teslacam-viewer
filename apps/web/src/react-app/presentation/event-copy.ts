/**
 * User-facing Japanese copy derived from domain values (DESIGN.md §6, §8; parity spec §5). Pure functions
 * only: components render these strings verbatim.
 */
import type { ClipCategory, EventMetadata, ScanProgress, ScanWarning, TeslaCamEvent } from '../../teslacam/types'
import { detectionSeconds, sentryInitialElapsedSeconds, sentryPlaybackStartLabel } from '../event-detection'
import { cameraLabels, formatDuration, type CameraAvailability } from '../event-summary'
import { eventClock } from '../event-filters'
import { parseFilenameLocalTimestamp, parseMetadataLocalTimestamp } from '../local-timestamp'

/** Detection earlier than this is not preceded by a pre-roll; playback starts at the beginning. */
export const PRE_ROLL_SECONDS = 15

export const shortCategoryLabels: Readonly<Record<ClipCategory, string>> = {
  recent: '走行',
  saved: '保存',
  sentry: 'セントリー',
}

/** `20:45` from a filename timestamp. */
export function startClock(timestamp: string): string {
  return eventClock(timestamp).slice(0, 5)
}

export function cameraCountText(availability: CameraAvailability): string {
  return `${availability.available}カメラ`
}

/** `右リピーターの記録がありません`, or null when every camera is present. */
export function missingCamerasText(availability: CameraAvailability): string | null {
  if (availability.missing.length === 0) return null
  return `${availability.missing.map((cameraId) => cameraLabels[cameraId]).join('、')}の記録がありません`
}

export type SentryDetection = {
  /** Seconds into the event (nominal 60 s per segment). */
  readonly seconds: number
  /** `20:35:30` */
  readonly timeLabel: string
  /** `物体の接近を`, or an empty string for reasons without words. */
  readonly reasonWords: string
}

function reasonWords(reason: string | undefined): string {
  if (reason === undefined) return ''
  if (reason.startsWith('sentry_aware_object_detection')) return '物体の接近を'
  if (reason.startsWith('sentry_aware_accel')) return '車体の揺れを'
  if (reason.includes('handle')) return 'ドアハンドルの操作を'
  return ''
}

/** Sentry detection inside the event; other categories never show a detection (spec header fix). */
export function sentryDetection(event: TeslaCamEvent): SentryDetection | null {
  if (event.category !== 'sentry') return null
  const seconds = detectionSeconds(event)
  const detected = parseMetadataLocalTimestamp(event.metadata?.timestamp ?? '')
  if (seconds === null || detected === null) return null
  return { seconds, timeLabel: detected.timeLabel, reasonWords: reasonWords(event.metadata?.reason) }
}

/** `20:35:30に検知` */
export function detectionFact(detection: SentryDetection): string {
  return `${detection.timeLabel}に検知`
}

/** `物体の接近を20:35:30に検知` */
export function detectionSentence(detection: SentryDetection): string {
  return `${detection.reasonWords}${detection.timeLabel}に検知`
}

/** True when playback of this event starts 15 s before a detection (not at the beginning). */
export function startsAtPreRoll(event: TeslaCamEvent): boolean {
  const detection = sentryDetection(event)
  return detection !== null && detection.seconds >= PRE_ROLL_SECONDS
}

/** `再生開始` detail: `20:35:15（検知の15秒前）`, or `20:35:00（記録の最初から）` for an early detection. */
export function playbackStartText(event: TeslaCamEvent): string | null {
  if (sentryDetection(event) === null) return null
  const label = sentryPlaybackStartLabel(event)
  if (label === null) return null
  const time = label.slice(11)
  return startsAtPreRoll(event) ? `${time}（検知の15秒前）` : `${time}（記録の最初から）`
}

/** Elapsed seconds where playback of a Sentry event starts. */
export function initialPlaybackSeconds(event: TeslaCamEvent): number {
  return sentryInitialElapsedSeconds(event)
}

export function locationText(metadata: EventMetadata | undefined): string | null {
  const parts = [metadata?.city?.trim() ?? '', metadata?.street?.trim() ?? ''].filter((part) => part.length > 0)
  return parts.length === 0 ? null : parts.join(' ')
}

export function coordinatesText(metadata: EventMetadata | undefined): string | null {
  const latitude = metadata?.est_lat?.trim() ?? ''
  const longitude = metadata?.est_lon?.trim() ?? ''
  return latitude.length > 0 && longitude.length > 0 ? `${latitude}, ${longitude}` : null
}

const weekdays = ['月曜日', '火曜日', '水曜日', '木曜日', '金曜日', '土曜日', '日曜日'] as const

/** `8月20日` and `木曜日` for a `YYYY-MM-DD` key (calendar arithmetic only, no time zones). */
export type DayTitle = {
  readonly month: number
  readonly day: number
  readonly date: string
  readonly weekday: string
}

export function dayTitle(dateKey: string): DayTitle | null {
  const parsed = parseFilenameLocalTimestamp(`${dateKey}_00-00-00`)
  if (parsed === null) return null
  // 0001-01-01 (ordinal day 0) is a Monday in the proleptic Gregorian calendar.
  const weekday = weekdays[Math.floor(parsed.ordinalSeconds / 86_400) % 7] ?? ''
  return { month: parsed.month, day: parsed.day, date: `${parsed.month}月${parsed.day}日`, weekday }
}

/** `M:SS` with unpadded minutes (`1:05`). */
export function transportTimeLabel(seconds: number): string {
  const whole = Math.max(0, Math.floor(seconds))
  return `${Math.floor(whole / 60)}:${String(whole % 60).padStart(2, '0')}`
}

function spokenTime(seconds: number, alwaysSeconds: boolean): string {
  const whole = Math.max(0, Math.floor(seconds))
  const minutes = Math.floor(whole / 60)
  const rest = whole % 60
  return rest === 0 && !alwaysSeconds ? `${minutes}分` : `${minutes}分${rest}秒`
}

/** Slider value text: `0分15秒 / 1分`. */
export function scrubberValueText(elapsedSeconds: number, durationSeconds: number): string {
  return `${spokenTime(elapsedSeconds, true)} / ${spokenTime(durationSeconds, false)}`
}

export function durationText(event: TeslaCamEvent): string {
  return formatDuration(event.segments.length * 60)
}

export type ScanProgressCopy = {
  /** Large numeral (`1,250` or `250 / 1,250`), absent before the first report. */
  readonly count: string | null
  readonly label: string
}

export function scanProgressCopy(progress: ScanProgress): ScanProgressCopy {
  if (progress.processed === 0) return { count: null, label: 'フォルダを読み込んでいます。' }
  const processed = progress.processed.toLocaleString('ja-JP')
  if (progress.processed === progress.total) return { count: processed, label: '件を確認しました' }
  return { count: `${processed} / ${progress.total.toLocaleString('ja-JP')}`, label: '件を確認しました' }
}

export function warningText(warning: ScanWarning): string {
  switch (warning.kind) {
    case 'empty-media-file':
      return `空の映像ファイル ${warning.count} 件を除外しました。`
    case 'invalid-metadata-file':
      return `読み取れないイベント情報 ${warning.count} 件を除外しました。`
    case 'unknown-file':
      return `対応外のファイル ${warning.count} 件を除外しました。`
    case 'encrypted-file':
      return `暗号化されたファイル ${warning.count} 件は読み込みませんでした。`
  }
}
