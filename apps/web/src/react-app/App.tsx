import { useReducer } from 'react'

import type { TeslaCamLibrary } from '../teslacam/types'
import { appReducer, initialAppState } from './app-state'
import { DirectoryInput } from './components/DirectoryInput'
import { LibraryPanel } from './components/library/LibraryPanel'
import { WelcomeContent } from './components/library/WelcomeContent'
import { ReadyWorkspace } from './components/ReadyWorkspace'
import { EmptyStage } from './components/viewer/EmptyStage'
import { useLayoutBand } from './hooks/use-layout-band'
import { useDirectoryScan } from './use-directory-scan'

const libraryKeys = new WeakMap<TeslaCamLibrary, number>()
let nextLibraryKey = 0

/** A new workspace (view mode, search, panes) for every loaded folder. */
function libraryKey(library: TeslaCamLibrary): number {
  const existing = libraryKeys.get(library)
  if (existing !== undefined) return existing
  nextLibraryKey += 1
  libraryKeys.set(library, nextLibraryKey)
  return nextLibraryKey
}

export function App() {
  const [state, dispatch] = useReducer(appReducer, initialAppState)
  const band = useLayoutBand()
  const { fallbackInputRef, isNativePickerAvailable, requestDirectory, handleDirectoryInput, cancelScan } = useDirectoryScan(dispatch)

  return (
    <>
      {state.kind === 'ready' ? (
        <ReadyWorkspace band={band} dispatch={dispatch} key={libraryKey(state.library)} state={state} onRescan={requestDirectory} />
      ) : (
        <div className="floor" data-band={band} data-phase="welcome">
          <LibraryPanel folderName={null}>
            <WelcomeContent
              compatibilityMode={!isNativePickerAvailable}
              error={state.kind === 'error' ? state.reason : null}
              progress={state.kind === 'scanning' ? state.progress : null}
              onCancelScan={cancelScan}
              onSelect={requestDirectory}
            />
          </LibraryPanel>
          {band !== 'phone' && <EmptyStage />}
        </div>
      )}
      <DirectoryInput inputRef={fallbackInputRef} nativePickerAvailable={isNativePickerAvailable} onDirectoryChange={handleDirectoryInput} />
    </>
  )
}
