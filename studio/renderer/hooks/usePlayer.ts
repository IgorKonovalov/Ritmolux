/**
 * The actions a view sends the player, named rather than composed at each call.
 *
 * A thin layer on purpose: every function here builds one `CtlAction` and hands
 * it to the bridge. The datagram is fire-and-forget because the wire is — OSC
 * has no acknowledgement — so nothing returns a promise and no view waits on
 * one. What comes back comes back as an event.
 *
 * The one exception is a preset selection, which goes out as `ctl/preset/req`
 * and is resent until the player's `preset_ack` names it (ADR-0265). Its
 * asker is one per window, not one per hook: two views selecting presets are
 * still one sender keeping one ask outstanding, and the acks arrive on the one
 * event subscription `usePlayerEvents` holds.
 */
import { useMemo } from 'react'

import { PresetAsker, type LostAsk } from '@shared/preset-ask'
import type { CtlAction, PresetAckEvent, PresetMark, TransportVerb } from '@shared/protocol'

export interface PlayerActions {
  /** Hold `value` on `name` until it is cleared or a reload drops it. */
  setParam: (name: string, value: number) => void
  clearParam: (name: string) => void
  clearParams: () => void
  /** Ask for `name`, superseding any selection still unanswered. */
  selectPreset: (name: string) => void
  transport: (verb: TransportVerb) => void
  /**
   * Put `mark` into the state `on` names on the preset called `name`.
   *
   * The studio never writes the marks file — the player is its only writer
   * (ADR-0229) — so nothing here is optimistic: the row moves when the `marks`
   * event comes back, exactly as it does for a mark made at the player's own
   * keyboard.
   */
  setMark: (name: string, mark: PresetMark, on: boolean) => void
}

const lostListeners = new Set<(lost: LostAsk) => void>()
let asker: PresetAsker | undefined

/** Created on first use, so importing this module touches no `window.api`. */
function presetAsker(): PresetAsker {
  asker ??= new PresetAsker({
    send: (action) => window.api.player.send(action),
    onLost: (lost) => {
      for (const listener of lostListeners) listener(lost)
    },
  })
  return asker
}

/** An ack off the event stream, for the window's one asker. */
export function presetAcked(event: PresetAckEvent): void {
  asker?.ack(event)
}

/** Hear every selection given up unanswered; returns the unsubscribe. */
export function onPresetAskLost(listener: (lost: LostAsk) => void): () => void {
  lostListeners.add(listener)
  return () => {
    lostListeners.delete(listener)
  }
}

export function usePlayerActions(): PlayerActions {
  return useMemo(() => {
    const send = (action: CtlAction): void => window.api.player.send(action)
    return {
      setParam: (name, value) => send({ kind: 'param', name, value }),
      clearParam: (name) => send({ kind: 'param_clear', name }),
      clearParams: () => send({ kind: 'params_clear' }),
      selectPreset: (name) => {
        presetAsker().ask(name)
      },
      transport: (verb) => send({ kind: 'transport', verb }),
      setMark: (name, mark, on) => send({ kind: 'mark', name, mark, on }),
    }
  }, [])
}
