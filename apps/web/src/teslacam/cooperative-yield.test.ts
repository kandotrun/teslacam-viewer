import { afterEach, vi } from 'vitest'

import { createYieldBudget, yieldToMain } from './cooperative-yield'

afterEach(() => {
  Reflect.deleteProperty(globalThis, 'scheduler')
})

describe('createYieldBudget', () => {
  it('stays on the current task until the time budget is spent', async () => {
    let now = 0
    const yieldNow = vi.fn(async () => undefined)
    const budget = createYieldBudget({ budgetMilliseconds: 10, now: () => now, yieldToMain: yieldNow })

    now = 9
    await budget.checkpoint()
    expect(yieldNow).not.toHaveBeenCalled()

    now = 10
    await budget.checkpoint()
    expect(yieldNow).toHaveBeenCalledTimes(1)

    now = 15
    await budget.checkpoint()
    expect(yieldNow).toHaveBeenCalledTimes(1)
  })
})

describe('yieldToMain', () => {
  it('prefers scheduler.yield when the browser offers it', async () => {
    const schedulerYield = vi.fn(async () => undefined)
    Object.defineProperty(globalThis, 'scheduler', { configurable: true, value: { yield: schedulerYield } })

    await yieldToMain()

    expect(schedulerYield).toHaveBeenCalledTimes(1)
  })

  it('falls back to a message channel task without timers', async () => {
    vi.useFakeTimers()
    try {
      await expect(yieldToMain()).resolves.toBeUndefined()
    } finally {
      vi.useRealTimers()
    }
  })
})
