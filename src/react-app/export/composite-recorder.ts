export type CompositeVideoExportErrorCode = 'unsupported' | 'no-playable-media' | 'aborted' | 'recording-failed'

export class CompositeVideoExportError extends Error {
  readonly code: CompositeVideoExportErrorCode

  constructor(code: CompositeVideoExportErrorCode, message: string) {
    super(message)
    this.name = 'CompositeVideoExportError'
    this.code = code
  }
}

export type CompositeRecorder = {
  readonly mimeType: string
  readonly state: RecordingState
  ondataavailable: ((event: BlobEvent) => void) | null
  onerror: ((event: ErrorEvent) => void) | null
  onpause: ((event: Event) => void) | null
  onresume: ((event: Event) => void) | null
  onstop: ((event: Event) => void) | null
  pause: () => void
  resume: () => void
  start: (timeslice?: number) => void
  stop: () => void
}

type FailureListener = (error: CompositeVideoExportError) => void

export function exportAborted(): CompositeVideoExportError {
  return new CompositeVideoExportError('aborted', '動画の書き出しをキャンセルしました。')
}

function recordingFailed(): CompositeVideoExportError {
  return new CompositeVideoExportError('recording-failed', '動画の録画に失敗しました。')
}

export class CompositeRecorderLifecycle {
  readonly chunks: Blob[] = []
  private readonly failureListeners = new Set<FailureListener>()
  private readonly recorder: CompositeRecorder
  private readonly signal: AbortSignal | undefined
  private failure: CompositeVideoExportError | undefined

  constructor(recorder: CompositeRecorder, signal?: AbortSignal) {
    this.recorder = recorder
    this.signal = signal
    this.recorder.ondataavailable = ({ data }) => {
      if (data.size > 0) this.chunks.push(data)
    }
    this.recorder.onerror = () => this.fail(recordingFailed())
    this.signal?.addEventListener('abort', this.handleAbort, { once: true })
    if (this.signal?.aborted === true) this.fail(exportAborted())
  }

  get mimeType(): string {
    return this.recorder.mimeType
  }

  get state(): RecordingState {
    return this.recorder.state
  }

  start(): void {
    this.ensureHealthy()
    try {
      this.recorder.start(1000)
    } catch (error) {
      if (error instanceof Error) {
        this.fail(recordingFailed())
        throw this.failure
      }
      throw error
    }
  }

  pause(): Promise<void> {
    if (this.recorder.state === 'paused') return Promise.resolve()
    return this.transition(
      (complete) => { this.recorder.onpause = complete },
      () => { this.recorder.onpause = null },
      () => this.recorder.pause(),
    )
  }

  resume(): Promise<void> {
    if (this.recorder.state === 'recording') return Promise.resolve()
    return this.transition(
      (complete) => { this.recorder.onresume = complete },
      () => { this.recorder.onresume = null },
      () => this.recorder.resume(),
    )
  }

  stop(): Promise<void> {
    if (this.recorder.state === 'inactive') return Promise.resolve()
    return this.transition(
      (complete) => { this.recorder.onstop = complete },
      () => { this.recorder.onstop = null },
      () => this.recorder.stop(),
    )
  }

  subscribeFailure(listener: FailureListener): () => void {
    if (this.failure !== undefined) listener(this.failure)
    else this.failureListeners.add(listener)
    return () => this.failureListeners.delete(listener)
  }

  forceStop(): void {
    if (this.recorder.state === 'inactive') return
    try {
      this.recorder.stop()
    } catch {
      this.fail(recordingFailed())
    }
  }

  dispose(): void {
    this.signal?.removeEventListener('abort', this.handleAbort)
    this.recorder.ondataavailable = null
    this.recorder.onerror = null
    this.recorder.onpause = null
    this.recorder.onresume = null
    this.recorder.onstop = null
    this.failureListeners.clear()
  }

  private readonly handleAbort = (): void => this.fail(exportAborted())

  private ensureHealthy(): void {
    if (this.failure !== undefined) throw this.failure
  }

  private fail(error: CompositeVideoExportError): void {
    if (this.failure !== undefined) return
    this.failure = error
    for (const listener of this.failureListeners) listener(error)
    this.failureListeners.clear()
  }

  private transition(
    install: (complete: () => void) => void,
    uninstall: () => void,
    action: () => void,
  ): Promise<void> {
    return new Promise((resolve, reject) => {
      if (this.failure !== undefined) {
        reject(this.failure)
        return
      }
      let settled = false
      const cleanup = () => {
        uninstall()
        this.failureListeners.delete(handleFailure)
      }
      const complete = () => {
        if (settled) return
        settled = true
        cleanup()
        resolve()
      }
      const handleFailure = (error: CompositeVideoExportError) => {
        if (settled) return
        settled = true
        cleanup()
        reject(error)
      }
      this.failureListeners.add(handleFailure)
      install(complete)
      try {
        action()
      } catch (error) {
        if (error instanceof Error) this.fail(recordingFailed())
        else throw error
      }
    })
  }
}
