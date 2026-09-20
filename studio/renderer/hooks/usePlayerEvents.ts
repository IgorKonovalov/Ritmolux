/**
 * The player's events, reduced to the facts a view needs.
 *
 * One subscription for the whole application: every event arrives on one
 * channel, and a component per event name would be four subscriptions to the
 * same stream.
 */
import { useEffect, useState } from 'react'

import type { HealthEvent, HelloEvent, PlayerEvent, StreamEvent } from '@shared/protocol'

/**
 * The user's opinion of the library, as the player last reported it.
 *
 * Two sets rather than a per-name flag: the event carries them whole, and
 * rebuilding a map per line would be a second shape to keep in step with a
 * first that is already correct.
 */
export interface Marks {
  favourite: string[]
  hidden: string[]
}

/**
 * A preset that failed or complained, as the banner shows it and as the editor
 * places it.
 *
 * `col` and `param` are carried because the editor needs one of them: a TOML
 * failure has a position, and an expression failure has only the parameter's
 * name, which is enough to find the line it is bound on (spec 0003).
 */
export interface Problem {
  file: string
  message: string
  line: number | null
  col: number | null
  param: string | null
  kind: 'error' | 'warning'
}

export interface PlayerState {
  hello?: HelloEvent
  stream?: StreamEvent
  health?: HealthEvent
  /**
   * The preset on screen, with the three facts an editor needs: the system's
   * canonical key, the family it draws, and the file it was read from — `null`
   * for the embedded set, which has none (ADR-0184).
   *
   * A player that reports no family at all is folded to `null` here, which is
   * the same answer as a system that has none: both mean "take a control's ends
   * from the single declared range".
   */
  preset?: {
    name: string
    index: number
    system: string
    file: string | null
    family: string | null
  }
  roster: string[]
  /**
   * The marks the player holds, or `undefined` until it has reported any.
   *
   * **`undefined` and two empty sets are different states** (ADR-0229): the
   * first is a studio with no player attached, which does not know which
   * library is loaded and therefore knows nothing about its marks; the second
   * is a player that says nothing is marked. A view that folded the two would
   * tell the user the library has no favourites when it has not been asked.
   */
  marks?: Marks
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
          family: event.family ?? null,
        },
      }
    case 'roster':
      return { ...state, roster: event.names, dir: event.dir, reloads: state.reloads + 1 }
    case 'marks':
      // Replaced whole, never merged: the line is the state, and a merge would
      // keep a name the player has just unmarked.
      return { ...state, marks: { favourite: event.favourite, hidden: event.hidden } }
    case 'preset_error':
      return {
        ...state,
        problems: [
          {
            file: event.file,
            message: event.message,
            line: event.line,
            col: event.col,
            param: event.param,
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
            col: null,
            param: event.param,
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
