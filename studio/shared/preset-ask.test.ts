/**
 * The sender rule spec 0003 states for `ctl/preset/req`: one ask outstanding,
 * the same `req` resent on a timeout, a newer ask supersedes, a bounded give-up.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { PresetAsker, type LostAsk } from './preset-ask'
import type { CtlAction } from './protocol'

type Sent = Extract<CtlAction, { kind: 'preset_req' }>

function rig(): { asker: PresetAsker; sent: Sent[]; lost: LostAsk[] } {
  const sent: Sent[] = []
  const lost: LostAsk[] = []
  const asker = new PresetAsker({
    send: (action) => sent.push(action),
    onLost: (ask) => lost.push(ask),
  })
  return { asker, sent, lost }
}

describe('PresetAsker', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })
  afterEach(() => {
    vi.useRealTimers()
  })

  it('sends once, resends the same id at 1 s and 2 s, and gives up once after the third', () => {
    const { asker, sent, lost } = rig()
    const req = asker.ask('aurora')
    expect(sent).toEqual([{ kind: 'preset_req', name: 'aurora', req }])

    vi.advanceTimersByTime(999)
    expect(sent).toHaveLength(1)
    vi.advanceTimersByTime(1)
    expect(sent).toHaveLength(2)
    vi.advanceTimersByTime(1000)
    expect(sent).toHaveLength(3)
    for (const action of sent) expect(action.req).toBe(req)
    expect(lost).toEqual([])

    vi.advanceTimersByTime(1000)
    expect(lost).toEqual([{ name: 'aurora', req, attempts: 3 }])
    expect(asker.pending).toBeUndefined()

    vi.advanceTimersByTime(10_000)
    expect(sent).toHaveLength(3)
    expect(lost).toHaveLength(1)
  })

  it('stops resending when an ack names the pending id', () => {
    const { asker, sent, lost } = rig()
    const req = asker.ask('aurora')
    vi.advanceTimersByTime(1000)
    asker.ack({ req })
    vi.advanceTimersByTime(10_000)
    expect(sent).toHaveLength(2)
    expect(lost).toEqual([])
    expect(asker.pending).toBeUndefined()
  })

  it('ignores an ack for a superseded id', () => {
    const { asker, sent, lost } = rig()
    const first = asker.ask('aurora')
    const second = asker.ask('gyre')
    expect(second).not.toBe(first)
    asker.ack({ req: first })
    expect(asker.pending).toBe(second)
    vi.advanceTimersByTime(1000)
    expect(sent.at(-1)).toEqual({ kind: 'preset_req', name: 'gyre', req: second })
    expect(lost).toEqual([])
  })

  it('cancels the first ask timer when a second selection supersedes it', () => {
    const { asker, sent, lost } = rig()
    asker.ask('aurora')
    vi.advanceTimersByTime(500)
    const second = asker.ask('gyre')
    vi.advanceTimersByTime(10_000)
    // Only the second ask's three sends follow the first's single one.
    expect(sent.filter((action) => action.name === 'aurora')).toHaveLength(1)
    expect(sent.filter((action) => action.name === 'gyre')).toHaveLength(3)
    expect(lost).toEqual([{ name: 'gyre', req: second, attempts: 3 }])
  })
})
