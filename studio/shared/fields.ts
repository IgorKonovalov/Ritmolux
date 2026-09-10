/**
 * Which control a schema kind gets, and how a value of that kind is written.
 *
 * A fact about the schema rather than about React, which is why it is here and
 * not in the component: the walk that asserts every kind the engine declares
 * resolves to something reads the document off the built player, and the
 * renderer may not open a file to do that (ADR-0178).
 */

/** How a key of one schema kind is written back into the file. */
export type FieldKind = 'number' | 'text' | 'enum' | 'bool' | 'colour' | 'readonly'

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
    case 'easing':
      return 'enum'
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

/** The TOML literal for a value of this kind, quoting decided by the kind. */
export function literalFor(kind: FieldKind, value: string): string {
  if (kind === 'number') {
    const parsed = Number(value)
    if (!Number.isFinite(parsed)) throw new Error(`\`${value}\` is not a number`)
    return String(parsed)
  }
  if (kind === 'bool') return value === 'true' ? 'true' : 'false'
  return JSON.stringify(value)
}
