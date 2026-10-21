import { useCallback, useReducer, useRef, type ReactNode } from 'react'

import { appReducer, initialAppState } from './app-state'
import { DirectoryInput } from './components/DirectoryInput'
import { EmptyState } from './components/EmptyState'
import { ReadyWorkspace } from './components/ReadyWorkspace'
import { ScanProgress } from './components/ScanProgress'
import { useDirectoryScan } from './use-directory-scan'

export function App() {
  const [state, dispatch] = useReducer(appReducer, initialAppState)
  const theaterRef = useRef<HTMLElement>(null)
  const {
    fallbackInputRef,
    isNativePickerAvailable,
    requestDirectory,
    handleDirectoryInput,
  } = useDirectoryScan(dispatch)

  const selectEvent = useCallback((eventId: string): void => {
    dispatch({ kind: 'event-selected', eventId })
    if (!globalThis.matchMedia('(max-width: 1024px)').matches) return
    requestAnimationFrame(() => theaterRef.current?.scrollIntoView({ block: 'start' }))
  }, [dispatch])

  let content: ReactNode
  switch (state.kind) {
    case 'idle':
      content = <EmptyState error={null} onSelect={requestDirectory} />
      break
    case 'scanning':
      content = <ScanProgress progress={state.progress} />
      break
    case 'error':
      content = <EmptyState error={state.reason} onSelect={requestDirectory} />
      break
    case 'ready':
      content = (
        <ReadyWorkspace
          dispatch={dispatch}
          state={state}
          theaterRef={theaterRef}
          onRescan={requestDirectory}
          onSelectEvent={selectEvent}
        />
      )
      break
  }

  return (
    <>
      {content}
      {!isNativePickerAvailable && state.kind !== 'ready' ? <CompatibilityStatus /> : null}
      <DirectoryInput inputRef={fallbackInputRef} onDirectoryChange={handleDirectoryInput} />
    </>
  )
}

function CompatibilityStatus() {
  return (
    <p aria-label="ブラウザ互換性" className="compatibility-status" role="status">
      このブラウザでは互換モードを使用します。フォルダ選択画面からTeslaCamフォルダを選んでください。
    </p>
  )
}
