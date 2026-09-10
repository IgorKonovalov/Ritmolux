/**
 * The preset file's `[params]` table, read and edited **by line**.
 *
 * Not a TOML parser. A preset carries long header comments that are the
 * project's own record, plus deliberate column alignment, and every
 * round-tripping library in reach reformats something; the contract this has to
 * meet is that a file written back unedited is byte-identical and a file with
 * one parameter moved differs on one line. A line editor meets that by
 * construction, because it never touches a line it is not editing.
 *
 * What it therefore does not do: multi-line values, dotted keys, a parameter
 * bound inside an inline table. A binding it cannot read is reported as
 * unreadable and shown rather than edited, which is the honest outcome — the
 * alternative is a writer that silently reformats a construct it half
 * understood.
 */

/**
 * One `[params]` entry, as the panel has to treat it.
 *
 * **Every binding is a quoted string.** The loader deserializes `[params]` into
 * a map of string to string and compiles each value as an expression, so a bare
 * TOML number there is a load error and not a shorter way to write a constant.
 * What distinguishes a constant is therefore the *expression*: a numeric
 * literal and nothing else.
 */
export type Binding =
  /** A numeric literal — the one shape a slider can move and write back. */
  | { kind: 'const'; name: string; value: number; line: number; quote: string }
  /** Any other expression: shown, never dragged — the file is what drives it. */
  | { kind: 'expr'; name: string; text: string; line: number }
  /** Not a quoted string at all, so not something this editor will rewrite. */
  | { kind: 'opaque'; name: string; text: string; line: number }

/** A table header, `[params]` or any other. */
const TABLE = /^\s*\[([^[\]]+)\]\s*(?:#.*)?$/
/**
 * `key = value`, with the value and any trailing comment left whole.
 *
 * The four capture groups are the file's own spacing: rewriting a value
 * substitutes group 4 and leaves 1, 3 and the comment exactly as they were, so
 * a preset's aligned `=` columns survive an edit.
 */
/*
 * The value is captured with a class that includes line terminators, not with
 * `.`: a carriage return is a line terminator to `.`, and a split on the line
 * feed leaves one at the end of every line of a CRLF file. A `.` here would
 * fail to reach the end anchor and match nothing, so every entry in every CRLF
 * preset would read as unbound.
 */
const ENTRY = /^(\s*)([A-Za-z_][A-Za-z0-9_-]*)(\s*=\s*)([\s\S]*)$/
/** A numeric literal, which is what a constant binding's expression is. */
const NUMBER = /^[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?$/
/** A single-line quoted value, with the quote character kept for the rewrite. */
const QUOTED = /^(["'])(.*)\1$/

/** Split a value cell into the value itself and whatever trails it. */
function splitTrailer(cell: string): { value: string; trailer: string } {
  // A `#` inside a quoted string is not a comment. Scan rather than regex,
  // because the quoted case is exactly the expression case and getting it
  // wrong would truncate an author's expression at a colour literal.
  let quote: string | undefined
  for (let i = 0; i < cell.length; i += 1) {
    const c = cell[i]
    if (quote !== undefined) {
      if (c === quote) quote = undefined
      continue
    }
    if (c === '"' || c === "'") {
      quote = c
      continue
    }
    if (c === '#') {
      const value = cell.slice(0, i).trimEnd()
      // From the end of the value, not from the `#`: the gap between them is
      // the author's column alignment and rewriting the value must not eat it.
      return { value, trailer: cell.slice(value.length) }
    }
  }
  const value = cell.trimEnd()
  return { value, trailer: cell.slice(value.length) }
}

/** Where one line's value cell starts, so a rewrite can splice it. */
interface Entry {
  name: string
  line: number
  indent: string
  equals: string
  value: string
  trailer: string
}

function scan(lines: string[]): { entries: Entry[]; paramsStart: number; paramsEnd: number } {
  let table: string | undefined
  let paramsStart = -1
  let paramsEnd = -1
  const entries: Entry[] = []
  for (let i = 0; i < lines.length; i += 1) {
    const header = TABLE.exec(lines[i])
    if (header !== null) {
      if (table === 'params') paramsEnd = i
      table = header[1].trim()
      if (table === 'params') paramsStart = i
      continue
    }
    if (table !== 'params') continue
    const entry = ENTRY.exec(lines[i])
    if (entry === null) continue
    const { value, trailer } = splitTrailer(entry[4])
    entries.push({
      name: entry[2],
      line: i,
      indent: entry[1],
      equals: entry[3],
      value,
      trailer,
    })
  }
  if (paramsStart !== -1 && paramsEnd === -1) paramsEnd = lines.length
  return { entries, paramsStart, paramsEnd }
}

/** Every binding the `[params]` table declares, in file order. */
export function readParams(text: string): Binding[] {
  const { entries } = scan(text.split('\n'))
  return entries.map((entry): Binding => {
    const quoted = QUOTED.exec(entry.value)
    if (quoted === null) {
      return { kind: 'opaque', name: entry.name, text: entry.value, line: entry.line }
    }
    const [, quote, expr] = quoted
    const literal = expr.trim()
    if (NUMBER.test(literal)) {
      return {
        kind: 'const',
        name: entry.name,
        value: Number(literal),
        line: entry.line,
        quote,
      }
    }
    return { kind: 'expr', name: entry.name, text: expr, line: entry.line }
  })
}

/**
 * A number as a preset writes one, inside the quotes.
 *
 * The shortest exact decimal, so a whole number reads `2` the way the shipped
 * presets write one. TOML's integer-versus-float distinction does not reach
 * here — the value is inside a string the expression compiler reads — so a
 * trailing `.0` would be noise in a diff rather than a type.
 *
 * Four decimals: finer than any slider step, and short enough that a person
 * reading the file afterwards sees a number rather than a float's tail.
 */
export function formatValue(value: number): string {
  if (!Number.isFinite(value)) throw new Error(`not a value a preset can hold: ${value}`)
  return String(Math.round(value * 1e4) / 1e4)
}

/** What the dominant line ending of this file is, so an inserted line matches. */
function lineEnding(lines: string[]): string {
  return lines.some((line) => line.endsWith('\r')) ? '\r' : ''
}

/**
 * `text` with `name` bound to `value`, and nothing else changed.
 *
 * Three cases, in order: the parameter has a line, so its value cell is
 * replaced and its indent, its `=` column and its trailing comment are kept;
 * the `[params]` table exists but does not bind it, so one line is appended to
 * that table; or there is no table, so one is appended to the file.
 *
 * Refuses to rewrite a binding it could not read — an array or an inline table
 * reaches here only if a caller ignored the `opaque` kind, and quietly
 * replacing it with a number would lose an author's work.
 */
export function setConstant(text: string, name: string, value: number): string {
  const lines = text.split('\n')
  const { entries, paramsStart, paramsEnd } = scan(lines)
  const eol = lineEnding(lines)
  const rendered = formatValue(value)

  const existing = entries.find((entry) => entry.name === name)
  if (existing !== undefined) {
    const quoted = QUOTED.exec(existing.value)
    if (quoted === null) {
      throw new Error(`\`${name}\` is bound to something this editor will not rewrite`)
    }
    // The file's own quote character, so a preset written with single quotes
    // does not acquire a double-quoted line in the middle of it.
    const quote = quoted[1]
    lines[existing.line] =
      `${existing.indent}${name}${existing.equals}${quote}${rendered}${quote}${existing.trailer}`
    return lines.join('\n')
  }

  const fresh = `${name} = "${rendered}"${eol}`
  if (paramsStart === -1) {
    // No table: append one. A trailing blank line is how these files end, so
    // the header goes after it rather than making a second one.
    const tail = lines.length > 0 && lines[lines.length - 1] === '' ? lines.length - 1 : lines.length
    lines.splice(tail, 0, `${eol}`, `[params]${eol}`, fresh)
    return lines.join('\n')
  }

  // After the table's last entry, or immediately after its header when it has
  // none, so the new line lands inside the table rather than after the blank
  // line that separates it from the next one.
  const inTable = entries.filter((entry) => entry.line > paramsStart && entry.line < paramsEnd)
  const at = inTable.length > 0 ? inTable[inTable.length - 1].line + 1 : paramsStart + 1
  lines.splice(at, 0, fresh)
  return lines.join('\n')
}
