/**
 * Every structural table the schema declares has an editor, and none of them is
 * listed here (Plan 0159 Phase 8).
 *
 * The walk runs over the document the **built player** printed, so a table or a
 * kind added to the engine reaches this test without an edit. With no built
 * player it falls back to a roster of the kinds the engine declares today —
 * enough to keep the resolver honest on a fresh clone, and it says which of the
 * two it used.
 *
 * It lives beside the resolver rather than beside the component because reading
 * the live document needs Node, which a file under `renderer/` may not have.
 */
import { execFileSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { parseSchemaDocument } from '../electron/player/schema'
import { editorForKind, literalFor } from './fields'
import type { SchemaDocument } from './schema'
import { structuralTables } from './templates'

/** Every kind the engine declares today, for the no-player fallback. */
const KINDS = [
  'expr',
  'table',
  'enum',
  'map',
  'colour',
  'int',
  'bool',
  'float',
  'easing',
  'list',
  'seed',
  'text',
]

function liveDocument(): SchemaDocument | undefined {
  const name = process.platform === 'win32' ? 'ritmolux.exe' : 'ritmolux'
  for (const profile of ['release', 'debug']) {
    const candidate = join(__dirname, '..', '..', 'target', profile, name)
    if (existsSync(candidate)) {
      return parseSchemaDocument(execFileSync(candidate, ['--schema'], { encoding: 'utf8' }))
    }
  }
  return undefined
}

const live = liveDocument()

/** The kinds to walk: the engine's own when there is one, else the roster above. */
function kinds(): string[] {
  if (live === undefined) {
    console.warn('skipped the live walk: no built ritmolux in target/; using the kind roster')
    return KINDS
  }
  return [
    ...new Set(structuralTables(live).flatMap((table) => table.keys.map((key) => key.kind))),
  ]
}

describe('the kind resolver', () => {
  it('resolves a control for every kind the schema declares', () => {
    const walked = kinds()
    expect(walked.length).toBeGreaterThan(6)
    for (const kind of walked) {
      expect(editorForKind(kind), `no editor for \`${kind}\``).toBeDefined()
    }
  })

  it('does not answer readonly for everything, which would make the walk vacuous', () => {
    const editable = kinds().filter((kind) => editorForKind(kind) !== 'readonly')
    expect(editable.length).toBeGreaterThan(4)
  })

  it('shows a composite rather than pretending to edit it', () => {
    // A list, a map and a nested table do not sit on one line, and a line
    // editor that rewrote one would reformat an author's array.
    for (const kind of ['list', 'map', 'table']) expect(editorForKind(kind)).toBe('readonly')
  })

  it('resolves something for a kind the engine has not invented yet', () => {
    // Total by construction: a newer player's kind is a weaker control, never
    // a blank row.
    expect(editorForKind('quaternion')).toBe('readonly')
  })
})

describe('the literal a kind is written as', () => {
  it('quotes what the file quotes and leaves bare what it does not', () => {
    expect(literalFor('number', '1.5')).toBe('1.5')
    expect(literalFor('bool', 'true')).toBe('true')
    expect(literalFor('enum', 'ember')).toBe('"ember"')
    expect(literalFor('colour', '#112233')).toBe('"#112233"')
    expect(literalFor('text', 'a "quoted" name')).toBe('"a \\"quoted\\" name"')
  })

  it('refuses a number that is not one, rather than writing NaN into a preset', () => {
    expect(() => literalFor('number', 'wobble')).toThrow(/not a number/)
  })
})
