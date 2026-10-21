import { createTaskPool, mapWithConcurrency } from './task-pool'

function deferred(): { readonly promise: Promise<void>; readonly resolve: () => void } {
  let resolve: () => void = () => undefined
  const promise = new Promise<void>((done) => {
    resolve = done
  })
  return { promise, resolve }
}

describe('createTaskPool', () => {
  it('never runs more tasks than its limit at once', async () => {
    const pool = createTaskPool(2)
    const gates = Array.from({ length: 5 }, deferred)
    let running = 0
    let peak = 0
    const results = gates.map((gate, index) => pool.run(async () => {
      running += 1
      peak = Math.max(peak, running)
      await gate.promise
      running -= 1
      return index
    }))

    await Promise.resolve()
    expect(running).toBe(2)
    for (const gate of gates) gate.resolve()

    await expect(Promise.all(results)).resolves.toEqual([0, 1, 2, 3, 4])
    expect(peak).toBe(2)
  })

  it('stops starting queued tasks once cancelled', async () => {
    const pool = createTaskPool(1)
    const gate = deferred()
    const started: number[] = []
    const first = pool.run(async () => {
      started.push(1)
      await gate.promise
    })
    const second = pool.run(async () => {
      started.push(2)
    })

    pool.cancel()
    gate.resolve()

    await first
    await expect(second).rejects.toThrow('cancelled')
    expect(started).toEqual([1])
  })
})

describe('mapWithConcurrency', () => {
  it('keeps input order and reports each outcome in order', async () => {
    const outcomes = await mapWithConcurrency([30, 10, 20], 2, async (delay) => {
      await new Promise((resolve) => setTimeout(resolve, delay))
      if (delay === 10) throw new Error('ten')
      return delay * 2
    })

    expect(outcomes).toEqual([
      { status: 'fulfilled', value: 60 },
      { status: 'rejected', reason: new Error('ten') },
      { status: 'fulfilled', value: 40 },
    ])
  })
})
