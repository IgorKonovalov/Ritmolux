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
  /**
   * The preset on screen, with the two facts an editor needs: the system's
   * canonical key, and the file it was read from — `null` for the embedded set,
   * which has none (ADR-0184).
   */
  preset?: { name: string; index: number; system: string; file: string | null }
  roster: string[]
  /** Where the watcher is looking, or `null` when nothing resolved. */
  dir: string | null
  /**
   * How many rosters have arrived.
   *
   * The player emits one on **every** reload, so this rises each time the file
   * on disk may have changed — which is the signal to re-read it. A counter
   * rather than a flag because a view reacts to the change, and two reloads in
   * a row must not look like one.
   */
  reloads: number
  /** Most recent first; the panel shows the head and the list keeps the rest. */
  problems: Problem[]
}

const EMPTY: PlayerState = { roster: [], dir: null, reloads: 0, problems: [] }

function reduce(state: PlayerState, event: PlayerEvent): PlayerState {
  switch (event.ev) {
    case 'hello':
      return { ...state, hello: event }
    case 'stream':
      return { ...state, stream: event }
    case 'health':
      return { ...state, health: event }
    case 'preset':
      return {
        ...state,
        preset: {
          name: event.name,
          index: event.index,
          system: event.system,
          file: event.file,
        },
      }
    case 'roster':
      return { ...state, roster: event.names, dir: event.dir, reloads: state.reloads + 1 }
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
