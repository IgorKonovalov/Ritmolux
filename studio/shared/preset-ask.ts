/**
 * One preset ask outstanding, resent until the player answers it (ADR-0265).
 *
 * A selection goes out as `ctl/preset/req` carrying a fresh `req`. Until a
 * `preset_ack` names that `req`, the same datagram is sent again every
 * `intervalMs`, for `attempts` sends in all; when the last one is unanswered
 * after its interval the ask is given up and reported once. A newer ask
 * supersedes the outstanding one: its timer is cleared and an ack that names it
 * afterwards is ignored.
 *
 * Resending is safe because the player treats a `req` ask for the preset it is
 * already landing on as inert and answers it `current`, so an ask that did land
 * and whose ack was the thing lost costs nothing when it arrives again.
 *
 * Pure: the send, the report and the clock are handed in, so the policy is
 * tested against fake timers and holds no handle of its own.
 */
import type { CtlAction, PresetAckEvent } from './protocol'

/** An ask no ack named within its attempts. */
export interface LostAsk {
  name: string
  req: number
  attempts: number
}

export interface PresetAskerOptions {
  send: (action: Extract<CtlAction, { kind: 'preset_req' }>) => void
  onLost: (lost: LostAsk) => void
  setTimer?: (run: () => void, ms: number) => unknown
  clearTimer?: (handle: unknown) => void
  intervalMs?: number
  attempts?: number
}

/** The studio's policy: every 1 s, three sends, then a give-up. */
export const PRESET_ASK_INTERVAL_MS = 1000
export const PRESET_ASK_ATTEMPTS = 3

/** OSC carries `req` as a signed 32-bit `i`; the id wraps back to 1 before overflowing it. */
const REQ_MAX = 2147483647

interface Outstanding {
  name: string
  req: number
  sent: number
  timer: unknown
}

export class PresetAsker {
  private readonly send: PresetAskerOptions['send']
  private readonly onLost: PresetAskerOptions['onLost']
  private readonly setTimer: (run: () => void, ms: number) => unknown
  private readonly clearTimer: (handle: unknown) => void
  private readonly intervalMs: number
  private readonly attempts: number
  private nextReq = 1
  private outstanding: Outstanding | undefined

  constructor(options: PresetAskerOptions) {
    this.send = options.send
    this.onLost = options.onLost
    this.setTimer = options.setTimer ?? ((run, ms) => setTimeout(run, ms))
    this.clearTimer =
      options.clearTimer ?? ((handle) => clearTimeout(handle as ReturnType<typeof setTimeout>))
    this.intervalMs = options.intervalMs ?? PRESET_ASK_INTERVAL_MS
    this.attempts = options.attempts ?? PRESET_ASK_ATTEMPTS
  }

  /** The `req` awaiting an ack, or `undefined` when none is. */
  get pending(): number | undefined {
    return this.outstanding?.req
  }

  /** Ask for `name`, superseding any ask still outstanding. Returns the `req` it carries. */
  ask(name: string): number {
    this.cancel()
    const req = this.nextReq
    this.nextReq = req === REQ_MAX ? 1 : req + 1
    this.outstanding = { name, req, sent: 0, timer: undefined }
    this.attempt()
    return req
  }

  /**
   * An ack off the event stream. Whatever its outcome, it ends the ask it names:
   * a `refused` one is reported by the `preset_error` the player raises beside
   * it, so nothing is surfaced here. An ack naming any other `req` is ignored.
   */
  ack(event: Pick<PresetAckEvent, 'req'>): void {
    if (this.outstanding === undefined || event.req !== this.outstanding.req) return
    this.cancel()
  }

  /** Drop the outstanding ask without reporting it. */
  cancel(): void {
    if (this.outstanding === undefined) return
    if (this.outstanding.timer !== undefined) this.clearTimer(this.outstanding.timer)
    this.outstanding = undefined
  }

  private attempt(): void {
    const ask = this.outstanding
    if (ask === undefined) return
    ask.sent += 1
    this.send({ kind: 'preset_req', name: ask.name, req: ask.req })
    ask.timer = this.setTimer(() => this.expire(ask), this.intervalMs)
  }

  private expire(ask: Outstanding): void {
    // A timer that fired after its ask was superseded or answered is stale.
    if (this.outstanding !== ask) return
    ask.timer = undefined
    if (ask.sent < this.attempts) {
      this.attempt()
      return
    }
    this.outstanding = undefined
    this.onLost({ name: ask.name, req: ask.req, attempts: ask.sent })
  }
}
