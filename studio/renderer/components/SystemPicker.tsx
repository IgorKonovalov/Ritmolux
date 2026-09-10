/**
 * Which system a preset drives, from the roster the engine declares.
 *
 * Changing it rewrites the `system` key, which is the one edit in the studio
 * that invalidates most of what is on screen: the parameters a preset may bind
 * are the new system's, and any it bound from the old one become load warnings.
 * So this says what is about to happen and does it on an explicit confirm
 * rather than on a stray scroll over a select.
 */
import { useState } from 'react'

import type { SchemaDocument } from '@shared/schema'

import styles from './SystemPicker.module.css'

export interface SystemPickerProps {
  document: SchemaDocument
  /** The key the player reported for the preset on screen. */
  current: string | undefined
  writable: boolean
  onChange: (system: string) => void
}

export function SystemPicker({
  document,
  current,
  writable,
  onChange,
}: SystemPickerProps): JSX.Element {
  const [chosen, setChosen] = useState<string>()
  const pending = chosen !== undefined && chosen !== current

  return (
    <section className={styles.picker}>
      <label className={styles.label} htmlFor="system">
        system
      </label>
      <select
        id="system"
        className={styles.select}
        value={chosen ?? current ?? ''}
        disabled={!writable}
        onChange={(event) => setChosen(event.currentTarget.value)}
      >
        {current === undefined && <option value="">no preset yet</option>}
        {document.systems.map((roster) => (
          <option key={roster.name} value={roster.name}>
            {roster.name}
          </option>
        ))}
      </select>
      {pending && (
        <>
          <p className={styles.warn}>
            A different system draws with different parameters. The ones this preset binds from{' '}
            <code>{current}</code> will be reported as warnings until they are removed.
          </p>
          <div className={styles.actions}>
            <button type="button" className={styles.confirm} onClick={() => onChange(chosen)}>
              change to {chosen}
            </button>
            <button type="button" className={styles.cancel} onClick={() => setChosen(undefined)}>
              keep {current}
            </button>
          </div>
        </>
      )}
    </section>
  )
}
