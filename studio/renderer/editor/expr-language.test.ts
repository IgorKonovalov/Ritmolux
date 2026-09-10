/**
 * The highlighter colours structure, and colours a name only when something
 * told it what that name is (Plan 0159 Phase 7).
 *
 * The plan asks for a token list **generated from the schema's function and
 * variable roster**. That roster does not exist: `--schema` declares systems,
 * stages and tables, and the engine's own `VAR_NAMES` and function match are
 * not exported. So the list is not generated here and it is not typed here
 * either — `presetLanguage` takes a roster and colours exactly what it is
 * given. These assertions pin that: with no roster, no identifier is a
 * function or a variable; with one, every name in it is and nothing else is.
 *
 * The day the schema carries the roster, the wiring is one argument and this
 * file becomes the test the plan asked for.
 */
import { StreamLanguage } from '@codemirror/language'
import { EditorState } from '@codemirror/state'
import { describe, expect, it } from 'vitest'

import { presetLanguage, type Grammar } from './expr-language'

/**
 * The token classes the mode assigns, in order, for one line.
 *
 * Runs the stream tokenizer directly rather than through a whole editor: what
 * is under test is the classification, and a `StreamLanguage`'s parser is the
 * thing that does it.
 */
function tokens(line: string, grammar?: Grammar): string[] {
  const language = presetLanguage(grammar)
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

const GRAMMAR: Grammar = {
  functions: ['sin', 'clamp'],
  variables: ['bass', 'time'],
}

describe('with no roster, which is what the studio has today', () => {
  it('colours a table header, a key, a number and a comment', () => {
    expect(tokens('[params]')).toContain('heading')
    expect(tokens('warp = "0.4"  # a note')).toEqual(
      expect.arrayContaining(['propertyName', 'string', 'comment']),
    )
  })

  it('colours no identifier inside an expression as a function or a variable', () => {
    const inside = tokens('warp = "sin(bass) + clamp(time, 0, 1)"')
    // Not "some names known and others not" — that is the state that tells an
    // author a real function is a typo.
    expect(inside).not.toContain('grammarFunction')
    expect(inside).not.toContain('grammarVariable')
  })
})

describe('with a roster, which is what it takes when the engine exports one', () => {
  it('colours every name the roster carries', () => {
    const inside = tokens('warp = "sin(bass)"', GRAMMAR)
    expect(inside).toContain('grammarFunction')
    expect(inside).toContain('grammarVariable')
  })

  it('colours no name the roster does not carry', () => {
    // `wobble` is not in GRAMMAR; if it were highlighted the mode would be
    // guessing, and a typo would look like a function.
    const inside = tokens('warp = "wobble(2)"', GRAMMAR)
    expect(inside).not.toContain('grammarFunction')
    expect(inside).not.toContain('grammarVariable')
  })

  it('still colours the numbers and the operators, which need no roster at all', () => {
    const inside = tokens('warp = "0.4 + 2"', GRAMMAR)
    expect(inside).toContain('number')
    expect(inside).toContain('operator')
  })
})

describe('the mode itself', () => {
  it('is a stream language the editor can install', () => {
    expect(presetLanguage()).toBeInstanceOf(StreamLanguage)
  })
})
