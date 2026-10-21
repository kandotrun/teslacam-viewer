/**
 * Cooperative scheduling for long local scans: work runs on the main thread in slices and gives the
 * browser a turn once a slice has used its time budget, instead of yielding every N items.
 */

type YieldBudgetOptions = {
  readonly budgetMilliseconds?: number
  readonly now?: () => number
  readonly yieldToMain?: () => Promise<void>
}

export type YieldBudget = {
  /** Resolves immediately while the slice is within budget, otherwise after a yield to the browser. */
  readonly checkpoint: () => Promise<void>
}

const defaultBudgetMilliseconds = 12

function schedulerYield(): (() => Promise<void>) | null {
  const scheduler: unknown = Reflect.get(globalThis, 'scheduler')
  if (typeof scheduler !== 'object' || scheduler === null) return null
  const yieldMethod: unknown = Reflect.get(scheduler, 'yield')
  if (typeof yieldMethod !== 'function') return null
  return async () => {
    await Reflect.apply(yieldMethod, scheduler, [])
  }
}

function messageChannelYield(): Promise<void> {
  return new Promise((resolve) => {
    const channel = new MessageChannel()
    channel.port1.onmessage = () => {
      channel.port1.close()
      resolve()
    }
    channel.port2.postMessage(null)
  })
}

/** Gives the browser a chance to render and handle input: `scheduler.yield()`, else a message task. */
export function yieldToMain(): Promise<void> {
  const preferred = schedulerYield()
  return preferred === null ? messageChannelYield() : preferred()
}

export function createYieldBudget(options: YieldBudgetOptions = {}): YieldBudget {
  const budget = options.budgetMilliseconds ?? defaultBudgetMilliseconds
  const now = options.now ?? (() => performance.now())
  const yieldNow = options.yieldToMain ?? yieldToMain
  let sliceStart = now()
  return {
    checkpoint: async () => {
      if (now() - sliceStart < budget) return
      await yieldNow()
      sliceStart = now()
    },
  }
}
