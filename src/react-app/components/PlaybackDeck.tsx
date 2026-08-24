import type { ReactNode } from 'react'

type PlaybackDeckProps = {
  readonly controls: ReactNode
  readonly timeline: ReactNode
}

export function PlaybackDeck({ controls, timeline }: PlaybackDeckProps) {
  return (
    <div className="playback-deck">
      {controls}
      {timeline}
    </div>
  )
}
