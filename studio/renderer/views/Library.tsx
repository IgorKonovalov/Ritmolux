/**
 * The roster the player loaded, and the two ways to add to it.
 *
 * The list is the `roster` event's, in the player's own order, and clicking one
 * sends `ctl/preset` — the player dissolves and then reports the preset on
 * screen, so the highlight follows what was drawn rather than what was asked
 * for. The two do differ for the length of a crossfade, and the one that is
 * true is the player's.
 *
 * A new preset is written from the schema's own defaults into the directory the
 * `roster` event named. The studio resolves no directory of its own (ADR-0184),
 * so with no directory there is nowhere to write and the form says so instead
 * of guessing.
 */
import { useState } from 'react'

import type { SchemaDocument } from '@shared/schema'
import { fileNameFor, templateFor } from '@shared/templates'

import styles from './Library.module.css'

export interface LibraryProps {
  document: SchemaDocument
  /** The preset names, in roster order. */
  roster: string[]
  /** The preset on screen, as the player last reported it. */
  active: string | undefined
  /** Where the watcher is looking, or `null` when nothing resolved. */
  dir: string | null
  /** The system a new preset starts as, taken from what is on screen. */
  system: string | undefined
  onSelect: (name: string) => void
  onCreate: (fileName: string, text: string) => void
  onProblem: (reason: string | undefined) => void
}

export function Library({
  document,
  roster,
  active,
  dir,
  system,
  onSelect,
  onCreate,
  onProblem,
}: LibraryProps): JSX.Element {
  const [name, setName] = useState('')
  const [chosen, setChosen] = useState<string>()
  const target = chosen ?? system ?? document.systems[0]?.name

  const create = (): void => {
    if (target === undefined) return
    try {
      onCreate(fileNameFor(name), templateFor(document, { name, system: target }))
      setName('')
      onProblem(undefined)
    } catch (error) {
      onProblem((error as Error).message)
    }
  }

  return (
    <div className={styles.library}>
      <ul className={styles.list}>
        {roster.map((preset) => (
          <li key={preset}>
            <button
              type="button"
              className={preset === active ? styles.entryOn : styles.entry}
              aria-current={preset === active}
              onClick={() => onSelect(preset)}
            >
              {preset}
            </button>
          </li>
        ))}
        {roster.length === 0 && <li className={styles.empty}>the player has reported no roster</li>}
      </ul>

      <form
        className={styles.new}
        onSubmit={(event) => {
          event.preventDefault()
          create()
        }}
      >
        <h3 className={styles.heading}>new preset</h3>
        {dir === null ? (
          <p className={styles.note}>
            This run resolved no preset directory, so it is drawing the built-in set and there is
            nowhere a new preset would be seen.
          </p>
        ) : (
          <>
            <input
              className={styles.name}
              value={name}
              placeholder="name"
              aria-label="new preset name"
              onChange={(event) => setName(event.currentTarget.value)}
            />
            <select
              className={styles.system}
              value={target ?? ''}
              aria-label="new preset system"
              onChange={(event) => setChosen(event.currentTarget.value)}
            >
              {document.systems.map((roster_) => (
                <option key={roster_.name} value={roster_.name}>
                  {roster_.name}
                </option>
              ))}
            </select>
            <button type="submit" className={styles.create} disabled={name.trim() === ''}>
              create in {dir}
            </button>
          </>
        )}
      </form>
    </div>
  )
}
