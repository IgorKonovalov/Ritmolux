/**
 * The editor colours exactly the names the engine says it knows
 * (Plan 0167 Phase 6, discharging Plan 0159 Phase 7).
 *
 * The roster is walked off the **built player's** own schema document, so a
 * function added to the grammar is coloured with no edit here and a name this
 * file invented would fail rather than pass. It **skips with a notice** when
 * there is no built player, which is ADR-0016's shape.
 *
 * It lives under `shared/` rather than beside `expr-language.ts` because
 * reading the live document needs Node, which nothing under `renderer/` may
 * import — the test files there included.
 */
import { execFileSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { join } from 'node:path'

import { EditorState } from '@codemirror/state'
import { describe, expect, it } from 'vitest'

import { parseSchemaDocument } from '../electron/player/schema'
import { presetLanguage } from '../renderer/editor/expr-language'
import type { SchemaDocument } from './schema'

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
  it('needs a built player to walk, and says so when there is none', () => {
    if (live === undefined) console.warn('skipped: no built ritmolux in target/')
    expect(true).toBe(true)
  })

  it('carries all three rosters, and none of them empty', () => {
    if (live === undefined) return
    const { variables, functions, constants } = live.grammar
    for (const [label, roster] of Object.entries({ variables, functions, constants })) {
      expect(roster.length, `the ${label} roster is empty`).toBeGreaterThan(0)
    }
  })

  it('names each identifier once, so a name cannot be two things at colouring time', () => {
    if (live === undefined) return
    const all = [...live.grammar.variables, ...live.grammar.functions, ...live.grammar.constants]
    expect(new Set(all).size).toBe(all.length)
  })
})

describe('what the editor colours', () => {
  it('colours every name the engine declares, whichever roster it is in', () => {
    if (live === undefined) return
    const expected: Record<string, string> = {}
    for (const name of live.grammar.variables) expected[name] = 'grammarVariable'
    for (const name of live.grammar.functions) expected[name] = 'grammarFunction'
    for (const name of live.grammar.constants) expected[name] = 'grammarConstant'

    for (const [name, want] of Object.entries(expected)) {
      expect(classify(`warp = "${name}"`, live), `\`${name}\` is not coloured`).toContain(want)
    }
  })

  it('colours no name the engine does not declare', () => {
    if (live === undefined) return
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
