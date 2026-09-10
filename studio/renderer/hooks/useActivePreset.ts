/**
 * The file behind the preset on screen: its text, its bindings, and the write.
 *
 * The path is the player's own (`preset.file`), never one the studio resolved —
 * a second resolver is what ADR-0184 refuses, and the failure it prevents is a
 * studio editing files the player is not watching while both report success.
 *
 * Re-read on every reload. The `roster` event fires on each one, so after a
 * write the text that comes back is what the player actually loaded rather than
 * what the studio believes it wrote; an edit made in another editor lands the
 * same way.
 */
import { useCallback, useEffect, useState } from 'react'

import { readParams, setConstant, type Binding } from '@shared/toml'

export type PresetFileState =
  /** No preset reported yet. */
  | { status: 'waiting' }
  /** On screen, but from the embedded set: there is no file to edit. */
  | { status: 'embedded' }
  | { status: 'loading'; path: string }
  | { status: 'ready'; path: string; text: string; bindings: Binding[] }
  | { status: 'failed'; path: string; reason: string }

export interface ActivePreset {
  file: PresetFileState
  /**
   * Write `name = value` into the file, atomically, and answer with the reason
   * it did not happen.
   *
   * The override is **not** cleared here: the player drops every override on a
   * reload (spec 0003), and clearing before the reload lands would snap the
   * picture back to the old value for as long as the watcher's poll takes.
   */
  commit: (name: string, value: number) => Promise<string | undefined>
}

export function useActivePreset(file: string | null | undefined, reloads: number): ActivePreset {
  const [state, setState] = useState<PresetFileState>({ status: 'waiting' })

  useEffect(() => {
    if (file === undefined) {
      setState({ status: 'waiting' })
      return
    }
    if (file === null) {
      setState({ status: 'embedded' })
      return
    }
    let live = true
    setState({ status: 'loading', path: file })
    void window.api.preset.read(file).then((result) => {
      if (!live) return
      setState(
        result.ok
          ? {
              status: 'ready',
              path: result.value.path,
              text: result.value.text,
              bindings: readParams(result.value.text),
            }
          : { status: 'failed', path: file, reason: result.reason },
      )
    })
    return () => {
      live = false
    }
  }, [file, reloads])

  const commit = useCallback(
    async (name: string, value: number): Promise<string | undefined> => {
      if (state.status !== 'ready') return 'there is no file behind this preset'
      let next: string
      try {
        next = setConstant(state.text, name, value)
      } catch (error) {
        return (error as Error).message
      }
      // Nothing to write when the value is already what the file says: a drag
      // that ends where it started must not touch an mtime and make the player
      // reload for nothing.
      if (next === state.text) return undefined
      const result = await window.api.preset.write(state.path, next)
      return result.ok ? undefined : result.reason
    },
    [state],
  )

  return { file: state, commit }
}
