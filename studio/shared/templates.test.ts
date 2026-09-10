/**
 * A template loads in the player (Plan 0159 Phase 8).
 *
 * The plan asks for `ritmolux --check`; there is no such flag, and its own
 * done-when names the fallback: drive the templates through a spawned player
 * and read its events. That is what the last block here does, with **one**
 * process for the whole set — a preset directory holding one template per
 * system, and a run whose `roster` must name every one of them and whose
 * `preset_error` must name none.
 *
 * It **skips with a notice** when there is no built player, no audio endpoint
 * or no usable adapter, which is ADR-0016's shape applied to a subprocess and
 * the same rule the Rust side's spawned tests follow. A studio test suite that
 * hard-required a compiled Rust binary would be red on every clone that has not
 * built one.
 */
import { execFileSync, spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { parseSchemaDocument } from '../electron/player/schema'
import type { SchemaDocument } from './schema'
import { canTemplate, fileNameFor, presetPath, structuralTables, templateFor } from './templates'

const ROOT = join(__dirname, '..', '..')

/** The built player, in either profile, or `undefined` on a fresh clone. */
function builtPlayer(): string | undefined {
  const name = process.platform === 'win32' ? 'ritmolux.exe' : 'ritmolux'
  for (const profile of ['release', 'debug']) {
    const candidate = join(ROOT, 'target', profile, name)
    if (existsSync(candidate)) return candidate
  }
  return undefined
}

/** The schema the built player prints, or `undefined` when there is none. */
function liveSchema(): SchemaDocument | undefined {
  const player = builtPlayer()
  if (player === undefined) return undefined
  return parseSchemaDocument(execFileSync(player, ['--schema'], { encoding: 'utf8' }))
}

const schema = liveSchema()

describe('a template', () => {
  it('needs a built player to be judged against, and says so when there is none', () => {
    if (schema === undefined) {
      console.warn('skipped: no built ritmolux in target/, so no schema to template from')
    }
    expect(true).toBe(true)
  })

  it('names the system it was asked for, and binds no parameter', () => {
    if (schema === undefined) return
    const text = templateFor(schema, { name: 'Probe', system: 'spectrum' })
    expect(text).toContain('system = "spectrum"')
    expect(text).toContain('name   = "Probe"')
    // Every parameter is already at the engine's default; writing them out
    // would bury the one line an author actually chose.
    expect(text.split('\n').filter((line) => /^\w+\s*=\s*"/.test(line))).toHaveLength(2)
  })

  it('refuses a system the player does not declare', () => {
    if (schema === undefined) return
    expect(canTemplate(schema, 'not_a_system')).toBe(false)
    expect(() => templateFor(schema, { name: 'X', system: 'not_a_system' })).toThrow(/not a system/)
  })

  it('turns a name into a file name that survives a filesystem', () => {
    expect(fileNameFor('Aurora Borealis')).toBe('aurora_borealis.toml')
    expect(fileNameFor('  ...Ember!  ')).toBe('ember.toml')
    expect(() => fileNameFor('   ')).toThrow(/a letter or a digit/)
  })

  it('joins a path with the separator the directory already uses', () => {
    expect(presetPath('C:\\Users\\a\\presets', 'x.toml')).toBe('C:\\Users\\a\\presets\\x.toml')
    expect(presetPath('/home/a/presets/', 'x.toml')).toBe('/home/a/presets/x.toml')
  })

  it('offers every table but the document and the palette stop as structural', () => {
    if (schema === undefined) return
    const names = structuralTables(schema).map((table) => table.name)
    expect(names).not.toContain('preset')
    expect(names).not.toContain('stop')
    expect(names).toContain('palette')
    expect(names.length).toBeGreaterThan(8)
  })
})

/**
 * The claim the plan actually asks for: the player loads them.
 *
 * One spawned run over a directory of templates, one per system. `--frames 1`
 * still opens an adapter and a capture, which is why the skip below exists.
 */
describe('every template, through the player', () => {
  it('loads with no error and no warning', () => {
    const player = builtPlayer()
    if (player === undefined || schema === undefined) {
      console.warn('skipped: no built ritmolux in target/')
      return
    }
    const dir = join(ROOT, 'target', 'tests', 'studio-templates')
    rmSync(dir, { recursive: true, force: true })
    mkdirSync(dir, { recursive: true })

    const expected: string[] = []
    for (const roster of schema.systems) {
      const name = `T ${roster.name}`
      writeFileSync(
        join(dir, fileNameFor(name)),
        templateFor(schema, { name, system: roster.name }),
        'utf8',
      )
      expected.push(name)
    }

    // `spawnSync` rather than `execFileSync`: the events are on standard
    // error, and the sync exec helpers hand back standard output.
    const run = spawnSync(
      player,
      ['--stream', '--sink', 'stdout', '--events', '--frames', '1', '--size', '160x90'],
      {
        // The per-user roots are cleared so the child cannot reach the
        // developer's real preset directory, config or diagnostics log.
        env: { ...process.env, RLX_PRESET_DIR: dir, APPDATA: '', HOME: '', XDG_DATA_HOME: '' },
        encoding: 'utf8',
        stdio: ['ignore', 'ignore', 'pipe'],
        timeout: 120_000,
      },
    )
    const stderr = run.stderr ?? ''
    if (
      stderr.includes('no audio capture device is available') ||
      (stderr.includes('--stream: ') && stderr.includes('adapter'))
    ) {
      console.warn('skipped: this machine has no capture endpoint or no usable adapter')
      return
    }
    expect(run.status, `the player did not run:\n${stderr}`).toBe(0)

    const lines = stderr.split('\n')
    const errors = lines.filter((line) => line.includes('"ev":"preset_error"'))
    const warnings = lines.filter((line) => line.includes('"ev":"preset_warning"'))
    expect(errors, `a template did not load:\n${errors.join('\n')}`).toEqual([])
    expect(warnings, `a template loaded with a problem:\n${warnings.join('\n')}`).toEqual([])

    const roster = lines.find((line) => line.includes('"ev":"roster"'))
    expect(roster, `the run reported no roster:\n${stderr}`).toBeDefined()
    for (const name of expected) expect(roster).toContain(name)

    rmSync(dir, { recursive: true, force: true })
  }, 300_000)
})
