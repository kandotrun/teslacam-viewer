/** Bounded concurrency for local file reads (file snapshots, `event.json`). */

export type TaskPool = {
  /** Starts the task as soon as fewer than `limit` tasks are running. */
  readonly run: <T>(task: () => Promise<T>) => Promise<T>
  /** Rejects every task that has not started yet. Running tasks finish normally. */
  readonly cancel: () => void
}

export type TaskOutcome<T> =
  | { readonly status: 'fulfilled'; readonly value: T }
  | { readonly status: 'rejected'; readonly reason: unknown }

type QueuedTask = {
  readonly start: () => void
  readonly reject: (reason: unknown) => void
}

export class TaskPoolCancelledError extends Error {
  readonly name = 'TaskPoolCancelledError'

  constructor() {
    super('Task pool cancelled')
  }
}

export function createTaskPool(limit: number): TaskPool {
  const queue: QueuedTask[] = []
  let running = 0
  let cancelled = false

  const startNext = (): void => {
    while (!cancelled && running < limit) {
      const next = queue.shift()
      if (next === undefined) return
      running += 1
      next.start()
    }
  }

  return {
    run: <T>(task: () => Promise<T>): Promise<T> => new Promise<T>((resolve, reject) => {
      if (cancelled) {
        reject(new TaskPoolCancelledError())
        return
      }
      queue.push({
        start: () => {
          task().then(resolve, reject).finally(() => {
            running -= 1
            startNext()
          })
        },
        reject,
      })
      startNext()
    }),
    cancel: () => {
      cancelled = true
      for (const queued of queue.splice(0)) queued.reject(new TaskPoolCancelledError())
    },
  }
}

/** Maps every item with at most `limit` calls in flight; outcomes keep the input order. */
export async function mapWithConcurrency<T, R>(
  items: readonly T[],
  limit: number,
  map: (item: T, index: number) => Promise<R>,
): Promise<readonly TaskOutcome<R>[]> {
  const pool = createTaskPool(limit)
  return Promise.all(items.map((item, index) => pool.run(() => map(item, index)).then(
    (value): TaskOutcome<R> => ({ status: 'fulfilled', value }),
    (reason: unknown): TaskOutcome<R> => ({ status: 'rejected', reason }),
  )))
}
