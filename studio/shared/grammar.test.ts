/**
 * The editor colours exactly the names the engine says it knows
 * (Plan 0167 Phase 6, discharging Plan 0159 Phase 7).
 *
 * The roster is walked off the **built player's** own schema document, so a
 * function added to the grammar is coloured with no edit here and a name this
 * file invented would fail rather than pass. With no built player — CI's studio
 * job builds none — it reads the committed `docs/specs/player-schema.json`,
 * which `core/tests/suite/preset_schema.rs` holds byte-equal to what
 * `ritmolux --schema` prints. **It never skips**: a missing snapshot fails the
 * file, because a walk over nothing passes every assertion.
 *
 * It lives under `shared/` rather than beside `expr-language.ts` because
 * reading the document needs Node, which nothing under `renderer/` may
 * import — the test files there included.
 */
import { execFileSync } from 'node:child_process'
import { existsSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

import { EditorState } from '@codemirror/state'
import { describe, expect, it } from 'vitest'

import { parseSchemaDocument } from '../electron/player/schema'
import { builtPlayer } from '../electron/testing/player'
import { presetLanguage } from '../renderer/editor/expr-language'
import type { SchemaDocument } from './schema'

const ROOT = join(__dirname, '..', '..')
const SNAPSHOT = join(ROOT, 'docs', 'specs', 'player-schema.json')

/** The built player's document, else the committed snapshot; throws with neither. */
function schemaDocument(): { document: SchemaDocument; source: string } {
  const player = builtPlayer()
  if (player.path !== undefined) {
    const text = execFileSync(player.path, ['--schema'], { encoding: 'utf8' })
    return { document: parseSchemaDocument(text), source: player.path }
  }
  if (!existsSync(SNAPSHOT)) {
    throw new Error(`${player.missing}, and no schema snapshot at ${SNAPSHOT}`)
  }
  return { document: parseSchemaDocument(readFileSync(SNAPSHOT, 'utf8')), source: SNAPSHOT }
}

const { document: live, source } = schemaDocument()

/** The token class the mode gives the single identifier in `line`. */
function classify(line: string, document: SchemaDocument): string[] {
  const language = presetLanguage(document.grammar)
  const state = EditorState.create({ doc: line, extensions: [language] })
  const tree = language.parser.parse(state.doc.toString())
  const out: string[] = []
  tree.iterate({
    enter: (node) => {
      if (node.name !== 'Document') out.push(node.name)
    },
  })
  return out
}

describe('the grammar the player exports', () => {
  it('carries all three rosters, and none of them empty', () => {
    const { variables, functions, constants } = live.grammar
    const names = variables.length + functions.length + constants.length
    console.info(`walking ${names} grammar names from ${source}`)
    for (const [label, roster] of Object.entries({ variables, functions, constants })) {
      expect(roster.length, `the ${label} roster is empty`).toBeGreaterThan(0)
    }
  })

  it('names each identifier once, so a name cannot be two things at colouring time', () => {
    const all = [...live.grammar.variables, ...live.grammar.functions, ...live.grammar.constants]
    expect(new Set(all).size).toBe(all.length)
  })
})

describe('what the editor colours', () => {
  it('colours every name the engine declares, whichever roster it is in', () => {
    const expected: Record<string, string> = {}
    for (const name of live.grammar.variables) expected[name] = 'grammarVariable'
    for (const name of live.grammar.functions) expected[name] = 'grammarFunction'
    for (const name of live.grammar.constants) expected[name] = 'grammarConstant'

    for (const [name, want] of Object.entries(expected)) {
      expect(classify(`warp = "${name}"`, live), `\`${name}\` is not coloured`).toContain(want)
    }
  })

  it('colours no name the engine does not declare', () => {
    // A typo must look like a typo. If this were coloured, an author would read
    // a misspelling as a function the engine has and wonder why it does nothing.
    const invented = 'wobblesnoot'
    expect([...live.grammar.variables, ...live.grammar.functions]).not.toContain(invented)
    const tokens = classify(`warp = "${invented}(2)"`, live)
    for (const tag of ['grammarVariable', 'grammarFunction', 'grammarConstant']) {
      expect(tokens).not.toContain(tag)
    }
  })
})
