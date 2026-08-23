import { useEffect, type Dispatch, type SetStateAction } from 'react'

import { cameraIds, type CameraId } from '../teslacam/types'

type TheaterShortcuts = {
  readonly expandedCamera: CameraId | null
  readonly setExpandedCamera: Dispatch<SetStateAction<CameraId | null>>
  readonly setActiveCamera: Dispatch<SetStateAction<CameraId>>
  readonly playing: boolean
  readonly play: () => Promise<void>
  readonly pause: () => void
  readonly skip: (seconds: number) => void
}

function isShortcutTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  return target.isContentEditable || ['INPUT', 'SELECT', 'BUTTON', 'TEXTAREA'].includes(target.tagName)
}

export function useTheaterShortcuts({
  expandedCamera,
  setExpandedCamera,
  setActiveCamera,
  playing,
  play,
  pause,
  skip,
}: TheaterShortcuts): void {
  useEffect(() => {
    const handleKey = (keyboardEvent: KeyboardEvent): void => {
      if (keyboardEvent.key === 'Escape' && expandedCamera !== null) {
        setExpandedCamera(null)
        keyboardEvent.preventDefault()
        return
      }
      if (isShortcutTarget(keyboardEvent.target)) return
      let handled = true
      switch (keyboardEvent.key) {
        case ' ':
          if (playing) pause()
          else void play()
          break
        case 'ArrowLeft':
          skip(-10)
          break
        case 'ArrowRight':
          skip(10)
          break
        case '1':
        case '2':
        case '3':
        case '4':
        case '5':
        case '6': {
          const cameraId = cameraIds[Number(keyboardEvent.key) - 1]
          if (cameraId !== undefined) {
            setActiveCamera(cameraId)
            setExpandedCamera((current) => current === cameraId ? null : cameraId)
          }
          break
        }
        default:
          handled = false
      }
      if (handled) keyboardEvent.preventDefault()
    }
    window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [expandedCamera, pause, play, playing, setActiveCamera, setExpandedCamera, skip])
}
