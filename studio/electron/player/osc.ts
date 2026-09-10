/**
 * The OSC 1.0 wire encoder, mirroring the player's.
 *
 * An address string, a type-tag string, then the arguments; every element is
 * NUL-terminated where it is a string and padded up to a 4-byte boundary.
 *
 * **The padding rule is the trap.** An OSC-string is always NUL-terminated and
 * *then* padded up, so a string whose length is already a multiple of 4 gains a
 * full 4 bytes rather than none: a 20-character address occupies 24 bytes on
 * the wire. `padTo4` is that rule and every element goes through it.
 *
 * Hand-rolled for the same reason the player's is: the encoder is smaller than
 * the justification a dependency would need. `osc.test.ts` holds it to packets
 * captured off the player's own encoder, so the two cannot drift on the wire.
 *
 * Numbers are big-endian, which OSC calls network byte order.
 */
import { ctlAddress, type CtlAction } from '@shared/protocol'

export type Arg =
  | { tag: 'f'; value: number }
  | { tag: 'i'; value: number }
  | { tag: 's'; value: string }

/**
 * Bytes of NUL padding that take `len` up to the next multiple of 4, **at least
 * one** — the terminator is part of what gets padded.
 */
function padTo4(len: number): number {
  return 4 - (len % 4)
}

/** `text` as an OSC-string: the bytes, a NUL, then padding to the boundary. */
function oscString(text: string): Buffer {
  // An interior NUL would make the receiver read a shorter string than was
  // written and mis-parse everything after it, so it is replaced rather than
  // passed through.
  const clean = text.replace(/\0/g, '?')
  const body = Buffer.from(clean, 'utf8')
  return Buffer.concat([body, Buffer.alloc(padTo4(body.length))])
}

/** One OSC message: address, type tags, arguments. */
export function encode(address: string, args: readonly Arg[]): Buffer {
  const parts: Buffer[] = [oscString(address), oscString(`,${args.map((a) => a.tag).join('')}`)]
  for (const arg of args) {
    switch (arg.tag) {
      case 'f': {
        const buf = Buffer.alloc(4)
        buf.writeFloatBE(arg.value, 0)
        parts.push(buf)
        break
      }
      case 'i': {
        const buf = Buffer.alloc(4)
        buf.writeInt32BE(arg.value, 0)
        parts.push(buf)
        break
      }
      case 's':
        parts.push(oscString(arg.value))
        break
    }
  }
  return Buffer.concat(parts)
}

/** The arguments each action carries, in the order its row declares them. */
export function ctlArgs(action: CtlAction): Arg[] {
  switch (action.kind) {
    case 'param':
      return [
        { tag: 's', value: action.name },
        { tag: 'f', value: action.value },
      ]
    case 'param_clear':
      return [{ tag: 's', value: action.name }]
    case 'params_clear':
      return []
    case 'preset':
      return [{ tag: 's', value: action.name }]
    case 'transport':
      return [{ tag: 's', value: action.verb }]
    case 'ping':
      return [{ tag: 'i', value: action.nonce }]
  }
}

/** One action as the datagram that carries it. */
export function encodeCtl(action: CtlAction): Buffer {
  return encode(ctlAddress(action), ctlArgs(action))
}
