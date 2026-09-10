/**
 * The encoder is byte-identical to the player's (Plan 0159 Phase 2).
 *
 * `shared/fixtures/osc-packets.json` is not written by hand: it is one packet
 * per telemetry address captured off a running player's own encoder over UDP.
 * The player *encodes* telemetry and only *decodes* `ctl`, so these are the
 * bytes it produces, and `ctl` messages go through the identical rules —
 * address, type tags, big-endian scalars, and the padding that catches
 * everyone.
 */
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { ctlAddress, type CtlAction } from '@shared/protocol'

import { encode, encodeCtl, type Arg } from './osc'

interface Packet {
  address: string
  hex: string
}

const packets: Packet[] = JSON.parse(
  readFileSync(join(__dirname, '..', '..', 'shared', 'fixtures', 'osc-packets.json'), 'utf8'),
) as Packet[]

function packet(address: string): Buffer {
  const found = packets.find((p) => p.address === address)
  if (found === undefined) throw new Error(`no captured packet for ${address}`)
  return Buffer.from(found.hex, 'hex')
}

/** Read the arguments back out of a captured packet, to re-encode them. */
function argsOf(buf: Buffer): Arg[] {
  const addressEnd = buf.indexOf(0)
  let at = addressEnd + (4 - (addressEnd % 4))
  const tagEnd = buf.indexOf(0, at)
  const tags = buf.subarray(at + 1, tagEnd).toString('ascii')
  const tagLen = tagEnd - at
  at += tagLen + (4 - (tagLen % 4))
  const args: Arg[] = []
  for (const tag of tags) {
    if (tag === 'f') {
      args.push({ tag: 'f', value: buf.readFloatBE(at) })
      at += 4
    } else if (tag === 'i') {
      args.push({ tag: 'i', value: buf.readInt32BE(at) })
      at += 4
    } else if (tag === 's') {
      const end = buf.indexOf(0, at)
      args.push({ tag: 's', value: buf.subarray(at, end).toString('utf8') })
      const len = end - at
      at += len + (4 - (len % 4))
    }
  }
  return args
}

describe('the encoder reproduces the player packets exactly', () => {
  it('captured a packet for every address the player publishes', () => {
    // If this drops, the capture ran against a player that publishes less and
    // the fixture is no longer evidence of anything.
    expect(packets.length).toBe(14)
  })

  it.each(packets.map((p) => p.address))('reproduces %s byte for byte', (address) => {
    const captured = packet(address)
    expect(encode(address, argsOf(captured))).toEqual(captured)
  })
})

describe('the padding rule the player states', () => {
  it('gives a full four bytes to a string already a multiple of four', () => {
    // `/rlx/v1/beat/trigger` is exactly 20 characters, and the player's packet
    // is 20 + 4 for the address before the type tag begins.
    const captured = packet('/rlx/v1/beat/trigger')
    expect(captured.subarray(20, 24)).toEqual(Buffer.alloc(4))
  })

  it('pads a string argument the same way', () => {
    // The preset name in this capture is `Clifford`, eight characters.
    const captured = packet('/rlx/v1/preset')
    const [arg] = argsOf(captured)
    expect(arg).toEqual({ tag: 's', value: 'Clifford' })
    expect(captured.subarray(captured.length - 4)).toEqual(Buffer.alloc(4))
  })

  it('makes every message a whole number of four-byte words', () => {
    for (const p of packets) expect(Buffer.from(p.hex, 'hex').length % 4).toBe(0)
  })
})

describe('the ctl vocabulary on the wire', () => {
  const actions: CtlAction[] = [
    { kind: 'param', name: 'warp', value: 0.5 },
    { kind: 'param_clear', name: 'warp' },
    { kind: 'params_clear' },
    { kind: 'preset', name: 'aurora' },
    { kind: 'transport', verb: 'next' },
    { kind: 'ping', nonce: 7 },
  ]

  it.each(actions)('encodes $kind to its address with its declared arguments', (action) => {
    const buf = encodeCtl(action)
    expect(buf.length % 4).toBe(0)
    const address = buf.subarray(0, buf.indexOf(0)).toString('ascii')
    expect(address).toBe(ctlAddress(action))
    // The type tags a decoder reads to know what follows.
    const tagAt = address.length + (4 - (address.length % 4))
    const tags = buf.subarray(tagAt, buf.indexOf(0, tagAt)).toString('ascii')
    expect(tags.startsWith(',')).toBe(true)
  })

  it('sends param as a string then a float, in that order', () => {
    const buf = encodeCtl({ kind: 'param', name: 'warp', value: 0.25 })
    expect(argsOf(buf)).toEqual([
      { tag: 's', value: 'warp' },
      { tag: 'f', value: 0.25 },
    ])
  })

  it('sends params/clear with no arguments at all', () => {
    const buf = encodeCtl({ kind: 'params_clear' })
    expect(argsOf(buf)).toEqual([])
  })

  it('replaces an interior NUL rather than truncating the message', () => {
    const buf = encodeCtl({ kind: 'preset', name: 'a\0b' })
    expect(argsOf(buf)).toEqual([{ tag: 's', value: 'a?b' }])
  })
})
