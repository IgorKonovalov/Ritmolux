/**
 * The control socket: one UDP sender aimed at the address the player answered
 * with.
 *
 * The target is never predicted. `--control 127.0.0.1:0` asks for an ephemeral
 * port, and the player reports what it actually bound in `hello.control` — or
 * reports `null`, when it opened no listener at all. Aiming from that answer is
 * what makes the sender correct in both cases instead of writing datagrams into
 * a port nothing is reading.
 *
 * OSC has no acknowledgement and no reply channel, so a send that goes nowhere
 * cannot be detected here; `ctl/ping` and the `pong` event are how a caller
 * tells a dead player from a quiet one, and the refusals below are the ones
 * this process can see for itself.
 */
import { createSocket, type Socket } from 'node:dgram'

import type { CtlAction } from '@shared/protocol'

import { encodeCtl } from './osc'

export interface ControlTarget {
  host: string
  port: number
}

/**
 * Split `host:port` as the player reports it.
 *
 * Returns `undefined` rather than throwing: the string arrives from a child
 * process, which makes it input to validate rather than a value to trust.
 */
export function parseControlAddress(address: string | null): ControlTarget | undefined {
  if (address === null) return undefined
  const at = address.lastIndexOf(':')
  if (at <= 0) return undefined
  const host = address.slice(0, at)
  const port = Number(address.slice(at + 1))
  if (!Number.isInteger(port) || port <= 0 || port > 65535) return undefined
  return { host, port }
}

export class ControlSender {
  private socket: Socket | undefined
  private target: ControlTarget | undefined
  private sentCount = 0
  private unaimedCount = 0

  /** Actions sent as datagrams. */
  get sent(): number {
    return this.sentCount
  }

  /** Actions dropped because the player named no listener to send them to. */
  get unaimed(): number {
    return this.unaimedCount
  }

  get aimedAt(): ControlTarget | undefined {
    return this.target
  }

  /** Point the sender at what `hello.control` reported. */
  aim(address: string | null): void {
    this.target = parseControlAddress(address)
  }

  send(action: CtlAction): boolean {
    if (this.target === undefined) {
      this.unaimedCount += 1
      return false
    }
    this.socket ??= createSocket('udp4')
    this.socket.send(encodeCtl(action), this.target.port, this.target.host)
    this.sentCount += 1
    return true
  }

  close(): void {
    this.socket?.close()
    this.socket = undefined
  }
}
