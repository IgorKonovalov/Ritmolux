/**
 * The actions a view sends the player, named rather than composed at each call.
 *
 * A thin layer on purpose: every function here builds one `CtlAction` and hands
 * it to the bridge. The datagram is fire-and-forget because the wire is — OSC
 * has no acknowledgement — so nothing returns a promise and no view waits on
 * one. What comes back comes back as an event.
 */
import { useMemo } from 'react'

import type { CtlAction, TransportVerb } from '@shared/protocol'

export interface PlayerActions {
  /** Hold `value` on `name` until it is cleared or a reload drops it. */
  setParam: (name: string, value: number) => void
  clearParam: (name: string) => void
  clearParams: () => void
  selectPreset: (name: string) => void
  transport: (verb: TransportVerb) => void
}

export function usePlayerActions(): PlayerActions {
  return useMemo(() => {
    const send = (action: CtlAction): void => window.api.player.send(action)
    return {
      setParam: (name, value) => send({ kind: 'param', name, value }),
      clearParam: (name) => send({ kind: 'param_clear', name }),
      clearParams: () => send({ kind: 'params_clear' }),
      selectPreset: (name) => send({ kind: 'preset', name }),
      transport: (verb) => send({ kind: 'transport', verb }),
    }
  }, [])
}
