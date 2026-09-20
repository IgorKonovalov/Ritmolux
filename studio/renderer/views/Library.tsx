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
 *
 * The marks are the player's too, and the studio opens no marks file of its own
 * (ADR-0229): a toggle sends `ctl/mark` and the row moves when the `marks` event
 * confirms it, which is also how a mark made at the player's keyboard arrives.
 * Until a player has reported any, there are no marks to show — not a library
 * with nothing marked, which is a different claim.
 */
import { useState } from 'react'

import type { PresetMark } from '@shared/protocol'
import type { SchemaDocument } from '@shared/schema'
import { fileNameFor, templateFor } from '@shared/templates'

import type { Marks } from '../hooks/usePlayerEvents'

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
  /** What the player says is marked, or `undefined` while none has said. */
  marks: Marks | undefined
  onSelect: (name: string) => void
  onMark: (name: string, mark: PresetMark, on: boolean) => void
  onCreate: (fileName: string, text: string) => void
  onProblem: (reason: string | undefined) => void
}

export function Library({
  document,
  roster,
  active,
  dir,
  system,
  marks,
  onSelect,
  onMark,
  onCreate,
  onProblem,
}: LibraryProps): JSX.Element {
  const [name, setName] = useState('')
  const [chosen, setChosen] = useState<string>()
  const [favouritesOnly, setFavouritesOnly] = useState(false)
  const target = chosen ?? system ?? document.systems[0]?.name

  const favourites = new Set(marks?.favourite ?? [])
  const hidden = new Set(marks?.hidden ?? [])
  /**
   * The narrowing holds only while the player is saying what is marked. With no
   * marks reported there is nothing to filter by, and a filter that quietly
   * emptied the list would read as a lost roster.
   */
  const narrowed = marks !== undefined && favouritesOnly
  const shown = narrowed ? roster.filter((preset) => favourites.has(preset)) : roster

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
      {marks === undefined ? (
        <p className={styles.note}>
          No player has reported its marks, so this list shows none. Marks live with the player, not
          with the studio.
        </p>
      ) : (
        <label className={styles.filter}>
          <input
            type="checkbox"
            checked={favouritesOnly}
            onChange={(event) => setFavouritesOnly(event.currentTarget.checked)}
          />
          favourites only ({favourites.size})
        </label>
      )}

      <ul className={styles.list}>
        {shown.map((preset) => (
          <li key={preset} className={hidden.has(preset) ? styles.rowHidden : styles.row}>
            <button
              type="button"
              className={preset === active ? styles.entryOn : styles.entry}
              aria-current={preset === active}
              onClick={() => onSelect(preset)}
            >
              {preset}
            </button>
            {/*
              A hidden preset keeps its row here. The player leaves it out of
              rotation and out of the browser's default view; the studio is where
              it is edited, and a preset the editing surface cannot reach is a
              mark that cannot be undone from the surface that set it.
            */}
            {marks !== undefined && (
              <span className={styles.marks}>
                <button
                  type="button"
                  className={favourites.has(preset) ? styles.markOn : styles.mark}
                  aria-label={`favourite ${preset}`}
                  aria-pressed={favourites.has(preset)}
                  onClick={() => onMark(preset, 'favourite', !favourites.has(preset))}
                >
                  ★
                </button>
                <button
                  type="button"
                  className={hidden.has(preset) ? styles.markOn : styles.mark}
                  aria-label={`hide ${preset}`}
                  aria-pressed={hidden.has(preset)}
                  onClick={() => onMark(preset, 'hidden', !hidden.has(preset))}
                >
                  ◦
                </button>
              </span>
            )}
          </li>
        ))}
        {roster.length === 0 && <li className={styles.empty}>the player has reported no roster</li>}
        {roster.length > 0 && shown.length === 0 && (
          <li className={styles.empty}>no preset is marked favourite</li>
        )}
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
