/**
 * The player's events, reduced to the facts a view needs.
 *
 * One subscription for the whole application: every event arrives on one
 * channel, and a component per event name would be four subscriptions to the
 * same stream.
 */
import { useEffect, useState } from 'react'

import type { HealthEvent, HelloEvent, PlayerEvent, StreamEvent } from '@shared/protocol'

/** A preset that failed or complained, as the banner and the list show it. */
export interface Problem {
  file: string
  message: string
  line: number | null
  kind: 'error' | 'warning'
}

export interface PlayerState {
  hello?: HelloEvent
  stream?: StreamEvent
  health?: HealthEvent
  preset?: { name: string; index: number }
  roster: string[]
  /** Most recent first; the panel shows the head and the list keeps the rest. */
  problems: Problem[]
}

const EMPTY: PlayerState = { roster: [], problems: [] }

function reduce(state: PlayerState, event: PlayerEvent): PlayerState {
  switch (event.ev) {
    case 'hello':
      return { ...state, hello: event }
    case 'stream':
      return { ...state, stream: event }
    case 'health':
      return { ...state, health: event }
    case 'preset':
      return { ...state, preset: { name: event.name, index: event.index } }
    case 'roster':
      return { ...state, roster: event.names }
    case 'preset_error':
      return {
        ...state,
        problems: [
          {
            file: event.file,
            message: event.message,
            line: event.line,
            kind: 'error',
          } satisfies Problem,
          ...state.problems,
        ].slice(0, 32),
      }
    case 'preset_warning':
      return {
        ...state,
        problems: [
          {
            file: event.file,
            message: event.message,
            line: null,
            kind: 'warning',
          } satisfies Problem,
          ...state.problems,
        ].slice(0, 32),
      }
    case 'pong':
      return state
  }
}

export function usePlayerEvents(): PlayerState {
  const [state, setState] = useState<PlayerState>(EMPTY)
  useEffect(() => window.api.player.onEvent((event) => setState((s) => reduce(s, event))), [])
  return state
}

export { reduce as reducePlayerEvent, EMPTY as emptyPlayerState }
