import { useCallback, useEffect, useReducer, useRef } from 'react'

import type { TeslaCamEvent } from '../../../teslacam/types'
import {
  exportCompositeVideo,
  resolveCompositeRecordingFormat,
} from '../../export/composite-video'
import {
  EXPORT_UNSUPPORTED_ERROR,
  exportFileName,
  exportReducer,
  initialExportState,
  readableExportError,
  type ExportState,
} from '../../export/export-state'

export type CompositeExportControl = {
  readonly state: ExportState
  readonly open: () => Promise<void>
  readonly closeDialog: () => void
  readonly start: () => Promise<void>
  readonly cancel: () => void
  readonly save: () => void
  readonly reset: () => void
}

/** Export job owned by one viewer session; it is aborted silently when the session ends. */
export function useCompositeExport(event: TeslaCamEvent): CompositeExportControl {
  const [state, dispatch] = useReducer(exportReducer, initialExportState)
  const abortController = useRef<AbortController | null>(null)

  useEffect(() => () => abortController.current?.abort(), [])

  const open = useCallback(async (): Promise<void> => {
    const format = await resolveCompositeRecordingFormat(event)
    if (format === null) {
      dispatch({ kind: 'failed', error: EXPORT_UNSUPPORTED_ERROR })
      return
    }
    dispatch({ kind: 'dialog-opened', format })
  }, [event])

  const start = useCallback(async (): Promise<void> => {
    const controller = new AbortController()
    abortController.current = controller
    dispatch({ kind: 'started' })
    try {
      const result = await exportCompositeVideo({
        event,
        signal: controller.signal,
        onProgress: (progress) => dispatch({ kind: 'progressed', progress }),
      })
      if (!controller.signal.aborted) dispatch({ kind: 'completed', result })
    } catch (error) {
      if (!controller.signal.aborted) dispatch({ kind: 'failed', error: readableExportError(error) })
    } finally {
      if (abortController.current === controller) abortController.current = null
    }
  }, [event])

  const cancel = useCallback((): void => {
    abortController.current?.abort()
    dispatch({ kind: 'cancelled' })
  }, [])

  const save = useCallback((): void => {
    if (state.status !== 'complete') return
    const url = URL.createObjectURL(state.result.blob)
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = exportFileName(event.timestamp, state.result.extension)
    anchor.click()
    setTimeout(() => URL.revokeObjectURL(url), 0)
  }, [event.timestamp, state])

  return {
    state,
    open,
    closeDialog: useCallback(() => dispatch({ kind: 'dialog-closed' }), []),
    start,
    cancel,
    save,
    reset: useCallback(() => dispatch({ kind: 'reset' }), []),
  }
}
