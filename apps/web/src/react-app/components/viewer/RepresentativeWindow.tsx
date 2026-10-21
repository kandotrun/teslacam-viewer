import { useEffect, useState } from 'react'

import type { LocalFileRef } from '../../../teslacam/types'
import type { ObjectUrlRegistry } from '../../playback/object-url-registry'

type RepresentativeWindowProps = {
  readonly fileRef: LocalFileRef
  readonly registry: ObjectUrlRegistry
}

/** `event.mp4` in one large window with native controls; it is not the synchronized six-camera clock. */
export function RepresentativeWindow({ fileRef, registry }: RepresentativeWindowProps) {
  const [url, setUrl] = useState<string | null>(null)
  const [unavailable, setUnavailable] = useState(false)

  useEffect(() => {
    let current = true
    let acquired: File | null = null
    void fileRef.getFile().then(
      (file) => {
        if (!current) return
        acquired = file
        setUrl(registry.acquire(file))
      },
      () => {
        if (current) setUnavailable(true)
      },
    )
    return () => {
      current = false
      if (acquired !== null) registry.release(acquired)
    }
  }, [fileRef, registry])

  return (
    <section aria-label="代表映像" className="representative">
      {url !== null && <video controls muted playsInline preload="metadata" src={url} />}
      {unavailable && <p className="representative__status" role="status">代表映像を読み込めません。</p>}
      <p className="representative__label">
        <strong>代表映像（単独再生）</strong>
        <span>6カメラ同期映像では<span className="nowrap">ありません。</span></span>
      </p>
    </section>
  )
}
