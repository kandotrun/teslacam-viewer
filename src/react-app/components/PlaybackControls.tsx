import { ChevronLeft, ChevronRight, Pause, Play, RotateCcw, RotateCw } from 'lucide-react'

import type { SynchronizedPlayback } from '../playback/use-synchronized-playback'

type PlaybackControlsProps = {
  readonly playback: SynchronizedPlayback
  readonly onPrevious: () => void
  readonly onNext: () => void
  readonly hasPrevious: boolean
  readonly hasNext: boolean
  readonly detectionSeconds: number | null
}

function formatTime(seconds: number): string {
  const minutes = Math.floor(seconds / 60)
  const remainder = Math.floor(seconds % 60)
  return `${minutes}:${String(remainder).padStart(2, '0')}`
}

export function PlaybackControls({ playback, onPrevious, onNext, hasPrevious, hasNext, detectionSeconds }: PlaybackControlsProps) {
  const detectionPercent = playback.durationSeconds === 0 || detectionSeconds === null
    ? null
    : detectionSeconds / playback.durationSeconds * 100
  return (
    <div className="playback-controls" aria-label="再生コントロール">
      <div className="playback-controls__primary">
        <button type="button" disabled={!hasPrevious} onClick={onPrevious} aria-label="前のイベント">
          <ChevronLeft aria-hidden="true" size={20} />
          前へ
        </button>
        <button type="button" onClick={() => playback.skip(-10)} aria-label="10秒戻る">
          <RotateCcw aria-hidden="true" size={19} />
          -10秒
        </button>
        <button type="button" className="playback-controls__play" onClick={() => playback.playing ? playback.pause() : void playback.play()} aria-label={playback.playing ? '一時停止' : '再生'}>
          {playback.playing ? <Pause aria-hidden="true" size={20} /> : <Play aria-hidden="true" size={20} />}
          {playback.playing ? '一時停止' : '再生'}
        </button>
        <button type="button" onClick={() => playback.skip(10)} aria-label="10秒進む">
          <RotateCw aria-hidden="true" size={19} />
          +10秒
        </button>
        <button type="button" disabled={!hasNext} onClick={onNext} aria-label="次のイベント">
          次へ
          <ChevronRight aria-hidden="true" size={20} />
        </button>
      </div>
      <div className="playback-controls__timeline">
        <span>{formatTime(playback.elapsedSeconds)}</span>
        <div className="playback-controls__range">
          <input
            aria-label="再生位置"
            type="range"
            min={0}
            max={playback.durationSeconds}
            step={0.1}
            value={playback.elapsedSeconds}
            onChange={(event) => playback.seek(event.currentTarget.valueAsNumber)}
          />
          {detectionPercent !== null && <span aria-label="検知位置" className="detection-marker" style={{ left: `${detectionPercent}%` }} />}
        </div>
        <span>{formatTime(playback.durationSeconds)}</span>
      </div>
      <label className="playback-controls__rate">
        <span>速度</span>
        <select
          aria-label="再生速度"
          value={String(playback.rate)}
          onChange={(event) => playback.setRate(Number(event.currentTarget.value))}
        >
          {[0.5, 1, 1.5, 2].map((speed) => (
            <option key={speed} value={String(speed)}>{speed}×</option>
          ))}
        </select>
      </label>
      {playback.message !== null && <p className="playback-message" role="status">{playback.message}</p>}
    </div>
  )
}
