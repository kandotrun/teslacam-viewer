import { ChevronLeft, ChevronRight, Ellipsis, Pause, Play, RotateCcw, RotateCw } from 'lucide-react'
import { useEffect, useId, useRef, useState } from 'react'

import type { SynchronizedPlayback } from '../../playback/use-synchronized-playback'
import { scrubberValueText, transportTimeLabel } from '../../presentation/event-copy'
import type { LayoutBand } from '../../presentation/stage-layout'

type TransportProps = {
  readonly playback: SynchronizedPlayback
  readonly band: LayoutBand
  /** Seconds of the Sentry detection; null for other categories (detection is Sentry-only). */
  readonly detectionSeconds: number | null
  /** Where playback started when it started at the pre-roll, else null. */
  readonly preRollSeconds: number | null
  readonly onOlder: (() => void) | null
  readonly onNewer: (() => void) | null
  /** The six-camera message is hidden while event.mp4 plays instead. */
  readonly showMessage?: boolean
}

const rates = [0.5, 1, 1.5, 2] as const

function percentOf(seconds: number, duration: number): number {
  return duration <= 0 ? 0 : Math.min(100, Math.max(0, seconds / duration * 100))
}

function SkipButtons({ playback }: { readonly playback: SynchronizedPlayback }) {
  return (
    <>
      <button aria-label="10秒戻る" className="icon-button transport__skip" type="button" onClick={() => playback.skip(-10)}>
        <RotateCcw aria-hidden="true" size={22} strokeWidth={1.6} />
        <small aria-hidden="true" className="num">10</small>
      </button>
      <button aria-label="10秒進む" className="icon-button transport__skip" type="button" onClick={() => playback.skip(10)}>
        <RotateCw aria-hidden="true" size={22} strokeWidth={1.6} />
        <small aria-hidden="true" className="num">10</small>
      </button>
    </>
  )
}

/** Tablet and phone: the skip buttons behind その他の操作; closes on Escape and on a press outside. */
function MoreMenu({ playback }: { readonly playback: SynchronizedPlayback }) {
  const [open, setOpen] = useState(false)
  const wrapper = useRef<HTMLDivElement>(null)
  const button = useRef<HTMLButtonElement>(null)
  const menuId = useId()

  useEffect(() => {
    if (!open) return undefined
    const closeOnOutside = (pointer: PointerEvent): void => {
      if (pointer.target instanceof Node && wrapper.current?.contains(pointer.target) === true) return
      setOpen(false)
    }
    document.addEventListener('pointerdown', closeOnOutside)
    return () => document.removeEventListener('pointerdown', closeOnOutside)
  }, [open])

  return (
    <div
      className="transport__more"
      ref={wrapper}
      onKeyDown={(keyboard) => {
        if (keyboard.key !== 'Escape' || !open) return
        keyboard.stopPropagation()
        setOpen(false)
        button.current?.focus()
      }}
    >
      <button
        aria-controls={menuId}
        aria-expanded={open}
        aria-label="その他の操作"
        className="icon-button"
        ref={button}
        type="button"
        onClick={() => setOpen((current) => !current)}
      >
        <Ellipsis aria-hidden="true" size={22} />
      </button>
      {open && (
        <div className="popover transport__menu" id={menuId}>
          <SkipButtons playback={playback} />
        </div>
      )}
    </div>
  )
}

/** The floating transport (DESIGN.md §6 Transport). */
export function Transport({ playback, band, detectionSeconds, preRollSeconds, onOlder, onNewer, showMessage = true }: TransportProps) {
  const duration = playback.durationSeconds
  const segmentTicks = Array.from({ length: Math.max(0, Math.round(duration / 60) - 1) }, (_, index) => percentOf((index + 1) * 60, duration))
  const detectionPercent = detectionSeconds === null || duration === 0 ? null : percentOf(detectionSeconds, duration)
  const preRollPercent = preRollSeconds === null || duration === 0 ? null : percentOf(preRollSeconds, duration)

  return (
    <footer aria-label="再生コントロール" className="transport" data-band={band}>
      <button aria-label="過去の記録" className="icon-button transport__older" disabled={onOlder === null} type="button" onClick={() => onOlder?.()}>
        <ChevronLeft aria-hidden="true" size={22} />
      </button>
      <button
        aria-label={playback.playing ? '一時停止' : '再生'}
        className="transport__play"
        type="button"
        onClick={() => {
          if (playback.playing) playback.pause()
          else void playback.play()
        }}
      >
        {playback.playing
          ? <Pause aria-hidden="true" fill="currentColor" size={24} strokeWidth={0} />
          : <Play aria-hidden="true" fill="currentColor" size={24} strokeWidth={0} />}
      </button>
      <div className="transport__clock">
        <span className="num transport__now">{transportTimeLabel(playback.elapsedSeconds)}</span>
        <span className="num transport__total">/ {transportTimeLabel(duration)}</span>
      </div>
      <div className="scrubber">
        <div className="scrubber__rail">
          <div aria-hidden="true" className="scrubber__track">
            <div className="scrubber__fill" style={{ width: `${percentOf(playback.elapsedSeconds, duration)}%` }} />
            {segmentTicks.map((percent) => <span className="scrubber__tick" key={percent} style={{ left: `${percent}%` }} />)}
          </div>
          {preRollPercent !== null && (
            <span className="scrubber__marker" data-kind="pre-roll" style={{ left: `${preRollPercent}%` }}>検知の15秒前</span>
          )}
          {detectionPercent !== null && (
            <span aria-label="検知位置" className="scrubber__marker" data-kind="detection" role="img" style={{ left: `${detectionPercent}%` }}>
              検知
            </span>
          )}
        </div>
        <input
          aria-label="再生位置"
          aria-valuetext={scrubberValueText(playback.elapsedSeconds, duration)}
          className="scrubber__input"
          max={duration}
          min={0}
          step={0.1}
          type="range"
          value={playback.elapsedSeconds}
          onChange={(event) => playback.seek(event.currentTarget.valueAsNumber)}
        />
      </div>
      <div className="transport__tools">
        {band !== 'desktop' ? (
          <MoreMenu playback={playback} />
        ) : (
          <SkipButtons playback={playback} />
        )}
        <label className="rate-pill">
          <span className="visually-hidden">再生速度</span>
          <select aria-label="再生速度" className="num" value={String(playback.rate)} onChange={(event) => playback.setRate(Number(event.currentTarget.value))}>
            {rates.map((rate) => <option key={rate} value={String(rate)}>{rate}×</option>)}
          </select>
        </label>
        <button aria-label="新しい記録" className="icon-button transport__newer" disabled={onNewer === null} type="button" onClick={() => onNewer?.()}>
          <ChevronRight aria-hidden="true" size={22} />
        </button>
      </div>
      {showMessage && playback.message !== null && <p className="transport__message" role="status">{playback.message}</p>}
    </footer>
  )
}
