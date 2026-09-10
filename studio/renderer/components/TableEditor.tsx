/**
 * One structural table, rendered from the keys the schema declares for it.
 *
 * **Nothing here is hand-listed.** The rows are the schema's `keys`, and which
 * control a row gets is decided by that key's `kind`. A table added to the
 * engine appears with no edit to this file; a kind added to the engine resolves
 * through `editorForKind` and is caught by the test that walks every kind the
 * document declares.
 *
 * The three composite kinds — a list, a map, a nested table — are shown and not
 * edited. A line editor rewrites the line a key sits on, and those three do not
 * sit on one line; a control that pretended otherwise would silently reformat
 * an author's array. That is a real limit and it is on screen, not hidden.
 */
import { editorForKind, literalFor } from '@shared/fields'
import type { TableKey, TableSpec } from '@shared/schema'
import type { Key } from '@shared/toml'

import styles from './TableEditor.module.css'

/** The value a control shows, from the literal the file holds. */
function display(literal: string | undefined, fallback: string): string {
  if (literal === undefined) return fallback
  const quoted = /^(["'])([\s\S]*)\1$/.exec(literal)
  return quoted === null ? literal : quoted[2]
}

export interface TableEditorProps {
  table: TableSpec
  /** The keys this preset's copy of the table declares. */
  keys: Key[]
  writable: boolean
  onSet: (table: string, key: string, literal: string) => void
}

export function TableEditor({ table, keys, writable, onSet }: TableEditorProps): JSX.Element {
  const held = new Map(keys.map((key) => [key.name, key]))

  return (
    <section className={styles.table}>
      <h3 className={styles.heading} title={table.doc}>
        {table.name}
      </h3>
      {table.keys.map((key) => (
        <Row
          key={key.name}
          spec={key}
          literal={held.get(key.name)?.literal}
          writable={writable}
          onSet={(literal) => onSet(table.name, key.name, literal)}
        />
      ))}
    </section>
  )
}

interface RowProps {
  spec: TableKey
  literal: string | undefined
  writable: boolean
  onSet: (literal: string) => void
}

function Row({ spec, literal, writable, onSet }: RowProps): JSX.Element {
  const kind = editorForKind(spec.kind)
  const value = display(literal, spec.default)
  const id = `key-${spec.name}`
  const set = (next: string): void => {
    try {
      onSet(literalFor(kind, next))
    } catch {
      // A half-typed number is not an error to report; the control keeps what
      // the file says until something valid is typed.
    }
  }

  return (
    <div className={styles.row} title={spec.doc}>
      <label className={styles.name} htmlFor={id}>
        {spec.name}
      </label>
      {kind === 'readonly' ? (
        <code className={styles.readonly}>{value === '' ? `(${spec.kind})` : value}</code>
      ) : kind === 'enum' ? (
        <select
          id={id}
          className={styles.control}
          value={value}
          disabled={!writable}
          onChange={(event) => set(event.currentTarget.value)}
        >
          {(spec.values ?? []).map((option) => (
            <option key={option} value={option}>
              {option}
            </option>
          ))}
        </select>
      ) : kind === 'bool' ? (
        <input
          id={id}
          className={styles.check}
          type="checkbox"
          checked={value === 'true'}
          disabled={!writable}
          onChange={(event) => set(event.currentTarget.checked ? 'true' : 'false')}
        />
      ) : (
        <input
          id={id}
          className={styles.control}
          type={kind === 'colour' ? 'color' : kind === 'number' ? 'number' : 'text'}
          value={value}
          disabled={!writable}
          onChange={(event) => set(event.currentTarget.value)}
        />
      )}
      {literal === undefined && <span className={styles.note}>default</span>}
    </div>
  )
}
