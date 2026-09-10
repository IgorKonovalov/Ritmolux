/**
 * Which control a schema kind gets, and how a value of that kind is written.
 *
 * A fact about the schema rather than about React, which is why it is here and
 * not in the component: the walk that asserts every kind the engine declares
 * resolves to something reads the document off the built player, and the
 * renderer may not open a file to do that (ADR-0178).
 */
import type { TableKey } from './schema'

/** How a key of one schema kind is written back into the file. */
export type FieldKind = 'number' | 'text' | 'enum' | 'bool' | 'colour' | 'scalar' | 'readonly'

/**
 * The control kind for a schema kind.
 *
 * Total by construction: anything the document declares that this does not
 * recognise resolves to `readonly`, which shows the value and says it cannot be
 * edited here. A missing case is therefore a weaker editor and never a blank
 * row or a crash.
 */
export function editorForKind(kind: string): FieldKind {
  switch (kind) {
    case 'float':
    case 'int':
    case 'seed':
      return 'number'
    case 'bool':
      return 'bool'
    case 'enum':
      return 'enum'
    // Two kinds whose value is a number *or* a word, and in `easing`'s case an
    // inline table as well. A `<select>` for either would offer a closed set
    // that does not exist; `scalar` writes the literal the value's own shape
    // asks for.
    case 'easing':
    case 'hold':
      return 'scalar'
    case 'colour':
      return 'colour'
    case 'text':
    case 'expr':
      return 'text'
    case 'list':
    case 'map':
    case 'table':
      return 'readonly'
    default:
      return 'readonly'
  }
}

/** The two words a `hold` edge accepts beside a number of seconds. */
export const HOLD_EDGES = ['beat', 'bar'] as const

/**
 * The control one entry of a `map` key gets, or `undefined` when the map is
 * shown rather than edited.
 *
 * A map's entries are author-named — `[hold]`'s keys are parameter names — so
 * the rows cannot come from the schema and the control for a row comes from
 * `of.kind` instead. Two element kinds are held back and both for a reason
 * rather than by name:
 *
 * - `expr` is the parameter panel's and the file tab's, which already give an
 *   expression a slider and a syntax-aware editor. A third, worse control for
 *   the same lines would compete with both.
 * - anything that resolves to `readonly` does not sit on one line, and a line
 *   editor that rewrote it would reformat an author's table.
 */
export function mapElementEditor(key: TableKey): FieldKind | undefined {
  if (key.kind !== 'map' || key.of === undefined) return undefined
  if (key.of.kind === 'expr') return undefined
  const kind = editorForKind(key.of.kind)
  return kind === 'readonly' ? undefined : kind
}

/** The TOML literal for a value of this kind, quoting decided by the kind. */
export function literalFor(kind: FieldKind, value: string): string {
  if (kind === 'number') {
    const parsed = Number(value)
    if (!Number.isFinite(parsed)) throw new Error(`\`${value}\` is not a number`)
    return String(parsed)
  }
  if (kind === 'bool') return value === 'true' ? 'true' : 'false'
  if (kind === 'scalar') return scalarLiteral(value)
  return JSON.stringify(value)
}

/**
 * A value whose TOML shape is decided by what was typed, not by its schema
 * kind: `2` is a number, `beat` is a string, and `{ attack = 0.1 }` is the
 * inline table the author already had there.
 *
 * The inline forms are passed through untouched rather than parsed. This is a
 * line editor: it can carry an author's `{ attack, release }` back into the
 * file unchanged, and quoting it would silently turn a working easing into a
 * string the loader refuses.
 */
function scalarLiteral(value: string): string {
  const text = value.trim()
  if (text === '') throw new Error('a value is required')
  if (text.startsWith('{') || text.startsWith('[')) return text
  const parsed = Number(text)
  return Number.isFinite(parsed) && text !== '' ? String(parsed) : JSON.stringify(text)
}
