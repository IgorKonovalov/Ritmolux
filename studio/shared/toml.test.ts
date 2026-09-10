/**
 * The line editor's contract: it changes the line it was asked to change, and
 * nothing else (Plan 0159 Phase 6).
 *
 * The round trip runs over **every shipped preset in this checkout**, not a
 * fixture. The header comments in those files are the project's own record, the
 * `=` columns are aligned by hand, and a writer that reflowed either would be a
 * regression no reviewer would see in a diff of the studio.
 */
import { readdirSync, readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { formatValue, readParams, setConstant } from './toml'

const PRESETS = join(__dirname, '..', '..', 'presets')

function shippedPresets(): { name: string; text: string }[] {
  return readdirSync(PRESETS)
    .filter((name) => name.endsWith('.toml'))
    .map((name) => ({ name, text: readFileSync(join(PRESETS, name), 'utf8') }))
}

/** The `(line, before, after)` triples on which two texts differ. */
function diff(before: string, after: string): { line: number; from: string; to: string }[] {
  const a = before.split('\n')
  const b = after.split('\n')
  const out: { line: number; from: string; to: string }[] = []
  for (let i = 0; i < Math.max(a.length, b.length); i += 1) {
    if (a[i] !== b[i]) out.push({ line: i, from: a[i] ?? '', to: b[i] ?? '' })
  }
  return out
}

describe('the shipped presets', () => {
  const presets = shippedPresets()

  it('finds a corpus to run against, so the assertions below are not vacuous', () => {
    expect(presets.length).toBeGreaterThan(20)
  })

  it('re-reads a constant binding this editor rewrote, as the value it wrote', () => {
    let checked = 0
    for (const { name, text } of presets) {
      const constant = readParams(text).find((binding) => binding.kind === 'const')
      if (constant === undefined) continue
      checked += 1
      const edited = setConstant(text, constant.name, 0.125)
      const again = readParams(edited).find((binding) => binding.name === constant.name)
      expect(again, `${name}: ${constant.name} vanished`).toEqual(
        expect.objectContaining({ kind: 'const', value: 0.125 }),
      )
    }
    expect(checked).toBeGreaterThan(10)
  })

  it('changes exactly one line, and only the value on it', () => {
    for (const { name, text } of presets) {
      const constant = readParams(text).find((binding) => binding.kind === 'const')
      if (constant === undefined) continue
      const changes = diff(text, setConstant(text, constant.name, 0.125))
      expect(changes.length, `${name}: ${changes.length} lines changed`).toBe(1)
      expect(changes[0].line).toBe(constant.line)
      // The name and everything after the value survive: only the number moved.
      expect(changes[0].to).toContain(constant.name)
      expect(changes[0].to).toContain('0.125')
    }
  })

  it('writes back byte for byte when the value does not move', () => {
    for (const { name, text } of presets) {
      const constant = readParams(text).find((binding) => binding.kind === 'const')
      if (constant === undefined) continue
      // The same value it already holds, through the same path an edit takes.
      const same = setConstant(text, constant.name, constant.value)
      const changes = diff(text, same)
      // A file whose author wrote `0.40` and not `0.4` differs in spelling, not
      // in value, so the one permitted change is that line and it must still
      // parse back to the same number.
      for (const change of changes) {
        expect(change.line, `${name}`).toBe(constant.line)
      }
      expect(readParams(same).find((b) => b.name === constant.name)).toEqual(
        expect.objectContaining({ value: constant.value }),
      )
    }
  })
})

describe('reading a binding', () => {
  const text = [
    '# a header comment the file keeps',
    'name = "Probe"',
    'system = "spectrum"',
    '',
    '[params]',
    'warp      = "0.4"      # aligned, with a trailing note',
    'bg_bright = "bins(3) * 2"',
    'seedy     = "-1.5e2"',
    'stops     = [1, 2, 3]',
    '',
    '[palette]',
    'name = "ember"',
    '',
  ].join('\n')

  it('tells a constant from an expression from something it will not touch', () => {
    expect(readParams(text)).toEqual([
      { kind: 'const', name: 'warp', value: 0.4, line: 5, quote: '"' },
      { kind: 'expr', name: 'bg_bright', text: 'bins(3) * 2', line: 6 },
      { kind: 'const', name: 'seedy', value: -150, line: 7, quote: '"' },
      { kind: 'opaque', name: 'stops', text: '[1, 2, 3]', line: 8 },
    ])
  })

  it('reads a bare TOML number as unrewritable, because the loader refuses one', () => {
    // `[params]` deserializes into a map of string to string, so `warp = 0.4`
    // is a load error and not a shorter constant. Rewriting one in place would
    // preserve a file that does not load.
    const bare = '[params]\nwarp = 0.4\n'
    expect(readParams(bare)).toEqual([
      { kind: 'opaque', name: 'warp', text: '0.4', line: 1 },
    ])
  })

  it('reads only the params table, not every key in the file', () => {
    expect(readParams(text).map((binding) => binding.name)).not.toContain('name')
  })

  it('keeps the alignment, the quotes and the trailing comment when it rewrites', () => {
    const edited = setConstant(text, 'warp', 0.9)
    expect(edited.split('\n')[5]).toBe('warp      = "0.9"      # aligned, with a trailing note')
  })

  it('keeps the quote character the file already used', () => {
    const single = "[params]\nwarp = '0.4'\n"
    expect(setConstant(single, 'warp', 0.9)).toBe("[params]\nwarp = '0.9'\n")
  })

  it('refuses to rewrite a binding it could not read', () => {
    expect(() => setConstant(text, 'stops', 1)).toThrow(/will not rewrite/)
  })

  it('adds a parameter the preset did not bind, inside the table it belongs to', () => {
    const edited = setConstant(text, 'zoom', 2)
    const lines = edited.split('\n')
    expect(lines[9]).toBe('zoom = "2"')
    expect(lines[10]).toBe('')
    expect(lines[11]).toBe('[palette]')
  })

  it('adds the table too when the preset has no params at all', () => {
    const bare = 'name = "Probe"\nsystem = "spectrum"\n'
    const edited = setConstant(bare, 'warp', 0.5)
    expect(edited).toBe('name = "Probe"\nsystem = "spectrum"\n\n[params]\nwarp = "0.5"\n')
  })

  it('reads and writes a file that uses carriage returns', () => {
    const crlf = ['[params]\r', 'warp = "0.4"\r', ''].join('\n')
    // The read half is the load-bearing one: a carriage return is a line
    // terminator to a regex `.`, so a pattern written the obvious way matches
    // nothing here and every parameter reads as unbound.
    expect(readParams(crlf)).toEqual([
      { kind: 'const', name: 'warp', value: 0.4, line: 1, quote: '"' },
    ])
    expect(setConstant(crlf, 'zoom', 1)).toBe(
      ['[params]\r', 'warp = "0.4"\r', 'zoom = "1"\r', ''].join('\n'),
    )
  })
})

describe('the value a preset is written with', () => {
  it('is the shortest exact decimal, the way the shipped presets write one', () => {
    expect(formatValue(1)).toBe('1')
    expect(formatValue(-2)).toBe('-2')
    expect(formatValue(0)).toBe('0')
  })

  it('is rounded to four decimals, which is finer than any slider step', () => {
    expect(formatValue(0.123456789)).toBe('0.1235')
    expect(formatValue(0.4)).toBe('0.4')
  })

  it('refuses a value no preset can hold', () => {
    expect(() => formatValue(Number.NaN)).toThrow()
    expect(() => formatValue(Number.POSITIVE_INFINITY)).toThrow()
  })
})
