/**
 * One author-named map — `[hold]`, `[smoothing]` — as editable rows.
 *
 * **The rows cannot come from the schema and the control can.** A map's keys
 * are the author's own: `[hold]`'s are parameter names, and which of them a
 * preset holds is a fact about that file. What one entry *accepts* is declared
 * — `of.kind` — so the control is resolved rather than chosen, and a map of a
 * new element kind gets a control here with no edit (ADR-0170).
 *
 * The suggestions on the add row are the parameters the active preset may bind,
 * which is the same roster the parameter panel is built from. They are
 * suggestions and not a closed list: an entry naming something the system does
 * not declare is refused by the engine, and it reports that as a
 * `preset_warning` the studio already shows.
 */
import { useState } from 'react'

import { HOLD_EDGES, literalFor, type FieldKind } from '@shared/fields'
import type { TableKey } from '@shared/schema'
import type { Key } from '@shared/toml'

import styles from './MapEditor.module.css'

/** The value a control shows, from the literal the file holds. */
function display(literal: string): string {
  const quoted = /^(["'])([\s\S]*)\1$/.exec(literal)
  return quoted === null ? literal : quoted[2]
}

export interface MapEditorProps {
  /** The TOML section this map is written as — `hold`, or `layer.hold`. */
  section: string
  spec: TableKey
  control: FieldKind
  /** The entries the preset's copy of this map declares, in file order. */
  entries: Key[]
  /** Names worth offering on the add row; not a closed set. */
  suggestions: string[]
  writable: boolean
  onSet: (section: string, key: string, literal: string) => void
  onRemove: (section: string, key: string) => void
}

export function MapEditor({
  section,
  spec,
  control,
  entries,
  suggestions,
  writable,
  onSet,
  onRemove,
}: MapEditorProps): JSX.Element {
  const [name, setName] = useState('')
  const [value, setValue] = useState('')
  const listId = `map-${section}-names`
  const edgesId = `map-${section}-edges`
  // The two words a hold edge accepts beside a number. Offered only where the
  // element kind is a hold; nothing else in the vocabulary has them.
  const edges = spec.of?.kind === 'hold' ? HOLD_EDGES : []

  const write = (key: string, next: string): void => {
    try {
      onSet(section, key, literalFor(control, next))
    } catch {
      // A half-typed value is not an error to report; the file keeps what it
      // has until something writable is typed.
    }
  }

  const add = (): void => {
    const key = name.trim()
    if (key === '' || value.trim() === '') return
    write(key, value)
    setName('')
    setValue('')
  }

  return (
    <section className={styles.map}>
      <h3 className={styles.heading} title={spec.doc}>
        [{section}]
      </h3>
      <p className={styles.doc}>{spec.doc}</p>

      {entries.length === 0 && <p className={styles.empty}>nothing held</p>}

      {entries.map((entry) => (
        <div className={styles.row} key={entry.name}>
          <label className={styles.name} htmlFor={`${section}-${entry.name}`}>
            {entry.name}
          </label>
          <input
            id={`${section}-${entry.name}`}
            className={styles.control}
            list={edges.length > 0 ? edgesId : undefined}
            defaultValue={display(entry.literal)}
            disabled={!writable}
            onBlur={(event) => write(entry.name, event.currentTarget.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') event.currentTarget.blur()
            }}
          />
          <button
            type="button"
            className={styles.remove}
            disabled={!writable}
            aria-label={`remove ${entry.name}`}
            onClick={() => onRemove(section, entry.name)}
          >
            &times;
          </button>
        </div>
      ))}

      <div className={styles.add}>
        <input
          className={styles.name}
          list={listId}
          placeholder="parameter"
          aria-label={`add to ${section}`}
          value={name}
          disabled={!writable}
          onChange={(event) => setName(event.currentTarget.value)}
        />
        <input
          className={styles.control}
          list={edges.length > 0 ? edgesId : undefined}
          placeholder={spec.of?.kind ?? 'value'}
          aria-label={`value for the new ${section} entry`}
          value={value}
          disabled={!writable}
          onChange={(event) => setValue(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === 'Enter') add()
          }}
        />
        <button type="button" className={styles.addButton} disabled={!writable} onClick={add}>
          add
        </button>
      </div>

      <datalist id={listId}>
        {suggestions.map((suggestion) => (
          <option key={suggestion} value={suggestion} />
        ))}
      </datalist>
      {edges.length > 0 && (
        <datalist id={edgesId}>
          {edges.map((edge) => (
            <option key={edge} value={edge} />
          ))}
        </datalist>
      )}
    </section>
  )
}
