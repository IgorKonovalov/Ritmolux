/**
 * Every kind the schema declares has a control, and none of them is listed here
 * (Plan 0159 Phase 8, deepened by Plan 0167 Phase 6).
 *
 * The walk runs over the document the **built player** printed, so a table, a
 * kind or a map element added to the engine reaches this test without an edit.
 * It **skips with a notice** when there is no built player, which is ADR-0016's
 * shape and the rule the studio's other player-dependent tests follow. There is
 * deliberately **no hand-kept fallback roster**: a list of kinds maintained here
 * is a second copy of the engine's vocabulary, and the first time it fell behind
 * it would report coverage the walk does not have — which is exactly what hid
 * `hold` until this phase.
 *
 * It lives beside the resolver rather than beside the component because reading
 * the live document needs Node, which a file under `renderer/` may not have.
 */
import { execFileSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { parseSchemaDocument } from '../electron/player/schema'
import { editorForKind, literalFor, mapElementEditor } from './fields'
import type { SchemaDocument, TableKey } from './schema'
import { structuralTables } from './templates'

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

/**
 * Every kind a key can present, **including the element kinds a composite
 * names**.
 *
 * A shallow walk over `key.kind` alone sees `map` and stops there, so the kind
 * a map's entries actually carry — `hold`, `easing`, `expr` — never reached the
 * resolver and never reached this test either.
 */
function elementKinds(key: TableKey): string[] {
  return key.of === undefined ? [key.kind] : [key.kind, key.of.kind]
}

function kinds(): string[] {
  if (live === undefined) return []
  return [...new Set(structuralTables(live).flatMap((table) => table.keys.flatMap(elementKinds)))]
}

/** Every map key the engine declares, across every structural table. */
function mapKeys(): TableKey[] {
  if (live === undefined) return []
  return structuralTables(live).flatMap((table) => table.keys.filter((key) => key.kind === 'map'))
}

describe('the kind resolver', () => {
  it('needs a built player to walk, and says so when there is none', () => {
    if (live === undefined) {
      console.warn('skipped the live walk: no built ritmolux in target/')
    }
    expect(true).toBe(true)
  })

  it('resolves a control for every kind the schema declares', () => {
    if (live === undefined) return
    const walked = kinds()
    expect(walked.length).toBeGreaterThan(6)
    for (const kind of walked) {
      expect(editorForKind(kind), `no editor for \`${kind}\``).toBeDefined()
    }
  })

  it('walks the element kinds a composite names, which is where hold lives', () => {
    if (live === undefined) return
    // Not an assertion about `hold` by name: it is the assertion that the walk
    // reaches past a composite at all, and `hold` is declared nowhere else.
    expect(kinds()).toContain('hold')
  })

  it('does not answer readonly for everything, which would make the walk vacuous', () => {
    if (live === undefined) return
    const editable = kinds().filter((kind) => editorForKind(kind) !== 'readonly')
    expect(editable.length).toBeGreaterThan(4)
  })

  it('shows a composite rather than pretending to edit it as one line', () => {
    // A list, a map and a nested table do not sit on one line, and a line
    // editor that rewrote one would reformat an author's array. A map's
    // *entries* are a different question, answered by `mapElementEditor`.
    for (const kind of ['list', 'map', 'table']) expect(editorForKind(kind)).toBe('readonly')
  })

  it('resolves something for a kind the engine has not invented yet', () => {
    // Total by construction: a newer player's kind is a weaker control, never
    // a blank row.
    expect(editorForKind('quaternion')).toBe('readonly')
  })
})

describe('the entries of a map', () => {
  it('gives every hold map a control, because the engine says what one entry is', () => {
    if (live === undefined) return
    const holds = mapKeys().filter((key) => key.of?.kind === 'hold')
    expect(holds.length).toBeGreaterThan(0)
    for (const key of holds) expect(mapElementEditor(key)).toBe('scalar')
  })

  it('leaves an expression map to the panel and the file tab', () => {
    if (live === undefined) return
    for (const key of mapKeys().filter((k) => k.of?.kind === 'expr')) {
      expect(mapElementEditor(key), `${key.name} should not be edited here`).toBeUndefined()
    }
  })

  it('leaves a map of tables alone, because an entry is not one line', () => {
    if (live === undefined) return
    for (const key of mapKeys().filter((k) => k.of?.kind === 'table')) {
      expect(mapElementEditor(key)).toBeUndefined()
    }
  })

  it('answers nothing for a key that is not a map at all', () => {
    expect(mapElementEditor({ name: 'warp', default: '', doc: '', kind: 'enum' })).toBeUndefined()
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

  it('lets a scalar be a number or a word, because a hold edge is either', () => {
    expect(literalFor('scalar', 'beat')).toBe('"beat"')
    expect(literalFor('scalar', '0.5')).toBe('0.5')
  })

  it('carries an author inline table back unchanged rather than quoting it', () => {
    // `{ attack = 0.1, release = 0.4 }` is a legal easing. Quoting it would
    // turn a working smoothing into a string the loader refuses, silently.
    const table = '{ attack = 0.1, release = 0.4 }'
    expect(literalFor('scalar', table)).toBe(table)
  })

  it('refuses a number that is not one, rather than writing NaN into a preset', () => {
    expect(() => literalFor('number', 'wobble')).toThrow(/not a number/)
    expect(() => literalFor('scalar', '   ')).toThrow(/required/)
  })
})
