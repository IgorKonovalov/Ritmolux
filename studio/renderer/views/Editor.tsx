/**
 * The editing surface: the preset on screen, and the parameters that move it.
 *
 * Everything it needs is a fact the player reported — which system to build the
 * panel for, and which file to write. Nothing here resolves a path, guesses a
 * system, or renders a row the engine did not declare.
 */
import { useCallback } from 'react'

import { rostersFor } from '@shared/schema'

import { ParamPanel } from '../components/ParamPanel'
import { useActivePreset } from '../hooks/useActivePreset'
import { usePlayerActions } from '../hooks/usePlayer'
import { useSchema } from '../hooks/useSchema'

import styles from './Editor.module.css'

export interface EditorProps {
  system: string | undefined
  /** The player's own path for the preset on screen; `null` for the embedded set. */
  file: string | null | undefined
  /** Rises on every reload, so the file is re-read after a save. */
  reloads: number
  onProblem: (reason: string | undefined) => void
}

export function Editor({ system, file, reloads, onProblem }: EditorProps): JSX.Element {
  const schema = useSchema()
  const actions = usePlayerActions()
  const { file: state, commit } = useActivePreset(file, reloads)

  const onDrag = useCallback(
    (name: string, value: number) => actions.setParam(name, value),
    [actions],
  )
  const onCommit = useCallback(
    (name: string, value: number) => {
      void commit(name, value).then(onProblem)
    },
    [commit, onProblem],
  )

  if (schema.status === 'loading') {
    return <p className={styles.note}>Reading the engine&apos;s parameter schema…</p>
  }
  if (schema.status === 'failed') {
    return (
      <p className={styles.note}>
        No panel: {schema.reason}. Every row here is generated from what the player declares, so
        there is nothing to show without it.
      </p>
    )
  }

  const rosters = rostersFor(schema.document, system)

  return (
    <div className={styles.editor}>
      <p className={styles.status}>
        {state.status === 'ready' && <span className={styles.path}>{state.path}</span>}
        {state.status === 'loading' && 'opening the preset file…'}
        {state.status === 'waiting' && 'waiting for the player to name a preset'}
        {state.status === 'embedded' &&
          'this preset is one of the built-in set and has no file — a drag moves the picture and nothing is written'}
        {state.status === 'failed' && `could not open ${state.path}: ${state.reason}`}
      </p>
      <ParamPanel
        rosters={rosters}
        bindings={state.status === 'ready' ? state.bindings : []}
        writable={state.status === 'ready'}
        onDrag={onDrag}
        onCommit={onCommit}
      />
    </div>
  )
}
