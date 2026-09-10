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

import {
  formatValue,
  readKeys,
  readPalette,
  readParams,
  removeKey,
  setConstant,
  setKey,
  setPaletteName,
  setStop,
} from './toml'

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
    expect(readParams(bare)).toEqual([{ kind: 'opaque', name: 'warp', text: '0.4', line: 1 }])
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

describe('the palette table', () => {
  const custom = [
    '[palette]',
    'stops = [',
    '  { at = 0.00, color = "#475a93" },  # lapis - the trunks',
    '  { at = 0.62, color = "#7fa650" },',
    ']',
    '',
  ].join('\n')

  it('reads the stops a preset declares, with the line each sits on', () => {
    expect(readPalette(custom)).toEqual({
      name: undefined,
      stops: [
        { at: 0, color: '#475a93', line: 2 },
        { at: 0.62, color: '#7fa650', line: 3 },
      ],
    })
  })

  it('reads a built-in palette by name', () => {
    expect(readPalette('[palette]\nname = "ember"\n')).toEqual({ name: 'ember', stops: [] })
  })

  it('finds no palette in a preset that declares none', () => {
    expect(readPalette('name = "Probe"\n')).toEqual({ name: undefined, stops: [] })
  })

  it('moves one stop and keeps the comment that names its colour', () => {
    const moved = setStop(custom, 0, { at: 0.25 })
    expect(moved.split('\n')[2]).toBe('  { at = 0.25, color = "#475a93" },  # lapis - the trunks')
    expect(moved.split('\n')[3]).toBe(custom.split('\n')[3])
  })

  it('recolours one stop and leaves its position alone', () => {
    const painted = setStop(custom, 1, { color: '#112233' })
    expect(painted.split('\n')[3]).toBe('  { at = 0.62, color = "#112233" },')
  })

  it('refuses a colour a preset cannot hold', () => {
    expect(() => setStop(custom, 0, { color: 'rebeccapurple' })).toThrow(/not a colour/)
    expect(() => setStop(custom, 0, { color: '#abc' })).toThrow(/not a colour/)
  })

  it('refuses a stop that is not there', () => {
    expect(() => setStop(custom, 7, { at: 0.5 })).toThrow(/no stop 7/)
  })

  it('names a built-in in a preset that has a palette table already', () => {
    expect(setPaletteName('[palette]\nname = "ember"\n', 'ice')).toBe('[palette]\nname = "ice"\n')
  })

  it('adds the table when the preset has no palette at all', () => {
    expect(setPaletteName('name = "Probe"\n', 'ice')).toBe(
      'name = "Probe"\n\n[palette]\nname = "ice"\n',
    )
  })

  it('refuses to name a built-in over a preset that authored its own stops', () => {
    // Both keys in one table is two palettes, and which wins is the loader's
    // business rather than something this editor decides by writing both.
    expect(() => setPaletteName(custom, 'ice')).toThrow(/clear them/)
  })

  it('changes exactly one line when it moves a stop in a shipped preset', () => {
    let checked = 0
    for (const { name, text } of shippedPresets()) {
      const palette = readPalette(text)
      if (palette.stops.length === 0) continue
      checked += 1
      const changes = diff(text, setStop(text, 0, { at: 0.11 }))
      expect(changes.length, `${name}: ${changes.length} lines changed`).toBe(1)
      expect(changes[0].line).toBe(palette.stops[0].line)
      expect(readPalette(setStop(text, 0, { at: 0.11 })).stops[0].at).toBe(0.11)
    }
    expect(checked).toBeGreaterThan(5)
  })
})

/**
 * An author-named map's entries are added, edited and removed one line at a
 * time (Plan 0167 Phase 6).
 *
 * `[hold]` is the case this exists for: its keys are parameter names the author
 * chose, so an editor can only work line by line, and the file around them is
 * the project's own record.
 */
const HELD = [
  '# A preset with a hold table.',
  'name   = "Probe"',
  'system = "spectrum"',
  '',
  '[hold]',
  'petals = "beat"   # on the downbeat',
  'warp   = 0.5',
  '',
  '[feedback]',
  'warp = "zoom"',
].join('\n')

describe('an author-named map table', () => {
  it('reads the entries the file holds, in file order', () => {
    expect(readKeys(HELD, 'hold').map((key) => key.name)).toEqual(['petals', 'warp'])
    expect(readKeys(HELD, 'hold')[0].literal).toBe('"beat"')
  })

  it('removes exactly the line the entry sits on', () => {
    const after = removeKey(HELD, 'hold', 'petals')
    expect(readKeys(after, 'hold').map((key) => key.name)).toEqual(['warp'])
    expect(after.split('\n').length).toBe(HELD.split('\n').length - 1)
    // The neighbours are untouched, comment column and all.
    expect(after).toContain('warp   = 0.5')
    expect(after).toContain('[feedback]')
  })

  it('leaves the header standing when the last entry goes', () => {
    let after = removeKey(HELD, 'hold', 'petals')
    after = removeKey(after, 'hold', 'warp')
    // An empty `[hold]` means what no `[hold]` means, and taking the header out
    // would take the blank line and any comment above it too.
    expect(after).toContain('[hold]')
    expect(readKeys(after, 'hold')).toEqual([])
  })

  it('is a no-op for an entry the file does not hold', () => {
    expect(removeKey(HELD, 'hold', 'drift')).toBe(HELD)
  })

  it('adds an entry into the table that is already there', () => {
    const after = setKey(HELD, 'hold', 'drift', '"bar"')
    expect(readKeys(after, 'hold').map((key) => key.name)).toEqual(['petals', 'warp', 'drift'])
    expect(after).toContain('[feedback]')
  })

  it('keeps an author comment column when it rewrites an entry', () => {
    const after = setKey(HELD, 'hold', 'petals', '"bar"')
    expect(after).toContain('petals = "bar"   # on the downbeat')
  })
})
