/**
 * The sender is aimed from the player's answer, never from a prediction
 * (Plan 0159 Phase 2).
 */
import { describe, expect, it } from 'vitest'

import { ControlSender, parseControlAddress } from './control'

describe('parseControlAddress', () => {
  it('splits what the player reports', () => {
    expect(parseControlAddress('127.0.0.1:54321')).toEqual({ host: '127.0.0.1', port: 54321 })
  })

  it('reads null as no listener at all', () => {
    expect(parseControlAddress(null)).toBeUndefined()
  })

  it('splits on the last colon, so a bracketed IPv6 host survives', () => {
    expect(parseControlAddress('[::1]:9000')).toEqual({ host: '[::1]', port: 9000 })
  })

  it('refuses what is not an address rather than half-reading it', () => {
    expect(parseControlAddress('127.0.0.1')).toBeUndefined()
    expect(parseControlAddress('127.0.0.1:')).toBeUndefined()
    expect(parseControlAddress('127.0.0.1:0')).toBeUndefined()
    expect(parseControlAddress('127.0.0.1:70000')).toBeUndefined()
    expect(parseControlAddress(':9000')).toBeUndefined()
  })
})

describe('ControlSender', () => {
  it('counts rather than sends while the player has named no listener', () => {
    const sender = new ControlSender()
    sender.aim(null)
    expect(sender.send({ kind: 'param', name: 'warp', value: 1 })).toBe(false)
    expect(sender.unaimed).toBe(1)
    expect(sender.sent).toBe(0)
    expect(sender.aimedAt).toBeUndefined()
    sender.close()
  })

  it('opens no socket at all until there is somewhere to send', () => {
    // A socket bound for a player that never opened a listener is a handle held
    // for nothing; `close()` on a sender that never sent must also not throw.
    const sender = new ControlSender()
    sender.aim(null)
    sender.send({ kind: 'params_clear' })
    expect(() => sender.close()).not.toThrow()
  })

  it('sends once aimed, and says where', async () => {
    const { createSocket } = await import('node:dgram')
    const receiver = createSocket('udp4')
    const arrived = new Promise<Buffer>((resolve) => receiver.once('message', resolve))
    await new Promise<void>((resolve) => receiver.bind(0, '127.0.0.1', resolve))

    const sender = new ControlSender()
    sender.aim(`127.0.0.1:${receiver.address().port}`)
    expect(sender.send({ kind: 'ping', nonce: 12 })).toBe(true)
    expect(sender.sent).toBe(1)

    const packet = await arrived
    expect(packet.subarray(0, packet.indexOf(0)).toString('ascii')).toBe('/rlx/v1/ctl/ping')
    expect(packet.readInt32BE(packet.length - 4)).toBe(12)

    sender.close()
    receiver.close()
  })
})
