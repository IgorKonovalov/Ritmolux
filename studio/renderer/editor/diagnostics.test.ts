/**
 * The player's complaints land on the lines they are about (Plan 0159 Phase 7).
 *
 * Driven from recorded events rather than from a live player, because what is
 * under test is the placement: given the line the player named, does the marker
 * cover that line's text, and does an expression failure — which carries no
 * position at all — still reach the line its parameter is bound on.
 */
import { describe, expect, it } from 'vitest'

import { markersFor, type PlayerProblem } from './diagnostics'

const FILE = 'C:/presets/aurora.toml'
const TEXT = [
  '# aurora',
  'name = "Aurora"',
  'system = "fragment_field"',
  '',
  '[params]',
  'warp = "0.4"',
  'bg_bright = "bins(3"',
  '',
].join('\n')

function problem(over: Partial<PlayerProblem> = {}): PlayerProblem {
  return {
    file: FILE,
    message: 'unexpected end of input',
    line: null,
    col: null,
    param: null,
    kind: 'error',
    ...over,
  }
}

describe('placing a problem', () => {
  it('marks the line a TOML failure named, and only that line', () => {
    // The event's line is one-based; line 7 is the broken expression.
    const [marker] = markersFor(TEXT, FILE, [problem({ line: 7 })])
    expect(TEXT.slice(marker.from, marker.to)).toBe('bg_bright = "bins(3"')
    expect(marker.severity).toBe('error')
    expect(marker.message).toBe('unexpected end of input')
  })

  it('marks the binding an expression failure named, which carries no position', () => {
    // An expression error is raised after the document was parsed into values
    // that no longer hold a span, so the parameter's name is all there is.
    const [marker] = markersFor(TEXT, FILE, [problem({ param: 'bg_bright' })])
    expect(TEXT.slice(marker.from, marker.to)).toBe('bg_bright = "bins(3"')
  })

  it('carries a warning through as a warning', () => {
    const [marker] = markersFor(TEXT, FILE, [problem({ line: 6, kind: 'warning' })])
    expect(marker.severity).toBe('warning')
  })

  it('clears when the problem does', () => {
    expect(markersFor(TEXT, FILE, [])).toEqual([])
  })

  it('drops a problem about a different preset in the same directory', () => {
    // A directory reloads as a whole, so an error in a sibling is routine;
    // marking it here would blame this document for another one's mistake.
    const other = markersFor(TEXT, FILE, [problem({ file: 'C:/presets/other.toml', line: 2 })])
    expect(other).toEqual([])
  })

  it('matches the same file across separator and case, which Windows varies', () => {
    const marked = markersFor(TEXT, 'C:\\presets\\Aurora.toml', [problem({ line: 7 })])
    expect(marked).toHaveLength(1)
  })

  it('drops a position the file no longer has, because the author kept typing', () => {
    // The events describe the file as the player last read it.
    expect(markersFor(TEXT, FILE, [problem({ line: 400 })])).toEqual([])
    expect(markersFor(TEXT, FILE, [problem({ param: 'not_bound_here' })])).toEqual([])
  })

  it('does not underline the carriage return of a CRLF file', () => {
    const crlf = TEXT.split('\n').join('\r\n')
    const [marker] = markersFor(crlf, FILE, [problem({ line: 7 })])
    expect(crlf.slice(marker.from, marker.to)).toBe('bg_bright = "bins(3"')
  })
})
