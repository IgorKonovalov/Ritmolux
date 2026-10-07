/**
 * The source directory's families, `proposed/` and file list, a pick resolved
 * to files, and the display name read back (Plan 0254 Phase 2).
 */
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { listSets, readDisplayName, resolveSet } from './sets'

function source(files: string[]): string {
  const dir = mkdtempSync(join(tmpdir(), 'rlx-judging-sets-'))
  for (const file of files) {
    mkdirSync(join(dir, file, '..'), { recursive: true })
    writeFileSync(join(dir, file), 'name = "x"\n')
  }
  return dir
}

describe('listSets', () => {
  it('lists the families, whether proposed/ exists, and every pickable file', () => {
    const dir = source([
      'attractor_a.toml',
      'attractor_b.toml',
      'curve_rose.toml',
      'lonely.toml',
      'README.md',
      'proposed/flow_x.toml',
      'pending/held_y.toml',
    ])
    expect(listSets(dir)).toEqual({
      dir,
      families: ['attractor', 'curve'],
      proposed: true,
      files: [
        'attractor_a.toml',
        'attractor_b.toml',
        'curve_rose.toml',
        'lonely.toml',
        'proposed/flow_x.toml',
      ],
    })
  })

  it('reads a missing directory as an empty one', () => {
    const dir = join(tmpdir(), 'rlx-judging-sets-nothing-here')
    expect(listSets(dir)).toEqual({ dir, families: [], proposed: false, files: [] })
  })
})

describe('resolveSet', () => {
  it('draws a family by its exact prefix', () => {
    const dir = source(['attractor_a.toml', 'attractors_b.toml', 'curve_c.toml'])
    expect(resolveSet(dir, { kind: 'family', prefix: 'attractor' })).toEqual([
      join(dir, 'attractor_a.toml'),
    ])
  })

  it('refuses an empty set and a list naming two files with one file name', () => {
    const dir = source(['a_x.toml', 'proposed/a_x.toml'])
    expect(resolveSet(dir, { kind: 'family', prefix: 'zzz' })).toHaveProperty('refused')
    expect(
      resolveSet(dir, { kind: 'list', files: ['a_x.toml', 'proposed/a_x.toml'] }),
    ).toHaveProperty('refused')
  })

  it('refuses a listed file that is missing or not a preset', () => {
    const dir = source(['a_x.toml'])
    expect(resolveSet(dir, { kind: 'list', files: ['a_y.toml'] })).toHaveProperty('refused')
    expect(resolveSet(dir, { kind: 'list', files: ['/etc/passwd'] })).toHaveProperty('refused')
  })
})

describe('readDisplayName', () => {
  it('reads the top-level name and nothing inside a table', () => {
    expect(readDisplayName('# c\nname = "Clifford"  # shown\nscene = "a"\n')).toBe('Clifford')
    expect(readDisplayName('scene = "a"\n[params]\nname = "Inner"\n')).toBeNull()
    expect(readDisplayName('name = ""\n')).toBeNull()
    expect(readDisplayName('name = "Esc\\"aped"\n')).toBeNull()
  })
})
