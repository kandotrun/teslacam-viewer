import { useEffect, useState } from 'react'

import type { TeslaCamEvent } from '../../teslacam/types'
import type { ObjectUrlRegistry } from '../playback/object-url-registry'

type RepresentativeVideoProps = {
  readonly event: TeslaCamEvent
  readonly registry: ObjectUrlRegistry
}

export function RepresentativeVideo({ event, registry }: RepresentativeVideoProps) {
  const [url, setUrl] = useState<string | null>(null)
  const [unavailable, setUnavailable] = useState(false)

  useEffect(() => {
    const fileRef = event.representativeFile
    if (fileRef === undefined) return undefined
    let active = true
    let acquired: File | null = null
    void fileRef.getFile().then(
      (file) => {
        if (!active) return
        acquired = file
        setUrl(registry.acquire(file))
      },
      () => {
        if (!active) return
        setUnavailable(true)
      },
    )
    return () => {
      active = false
      if (acquired !== null) registry.release(acquired)
    }
  }, [event.representativeFile, registry])

  return (
    <section className="representative-video" aria-label="代表映像">
      <div>
        <strong>代表映像（単独再生）</strong>
        <span>6カメラ同期映像では<span className="representative-video__tail">ありません。</span></span>
      </div>
      {unavailable && <span role="status">代表映像を読み込めません。</span>}
      {url !== null && <video controls muted playsInline preload="metadata" src={url} />}
    </section>
  )
}
