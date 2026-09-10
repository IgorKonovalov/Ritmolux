/**
 * A fork lands on a name nothing is using, or it does not land (Plan 0168
 * Phase 1).
 *
 * The write half is Plan 0159's and unchanged: a temporary file beside the
 * target, then a rename, so the watcher's poll never reads half a document. The
 * create half adds the one refusal ADR-0189 turns on — a preset the studio did
 * not make is not overwritten, and the proof is its bytes afterwards.
 */
import { mkdtemp, readFile, readdir, writeFile, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { afterEach, beforeEach, describe, expect, it } from 'vitest'

import { createPresetFile, readPreset, writePresetAtomically } from './writer'

let dir: string

beforeEach(async () => {
  dir = await mkdtemp(join(tmpdir(), 'rlx-writer-'))
})

afterEach(async () => {
  await rm(dir, { recursive: true, force: true })
})

const CURATED = '# ink\n\nname   = "ink"\nsystem = "attractor"\n'

describe('creating a preset', () => {
  it('writes the document and leaves no temporary file behind', async () => {
    await createPresetFile(join(dir, 'mine.toml'), CURATED)
    expect(await readFile(join(dir, 'mine.toml'), 'utf8')).toBe(CURATED)
    expect(await readdir(dir)).toEqual(['mine.toml'])
  })

  it('refuses a name that is already a preset, byte for byte', async () => {
    const taken = join(dir, 'ink.toml')
    await writeFile(taken, CURATED, 'utf8')

    await expect(createPresetFile(taken, 'name = "not this"\n')).rejects.toThrow(/already there/)
    // The whole point: the curated file is untouched, not merely unopened.
    expect(await readFile(taken, 'utf8')).toBe(CURATED)
    expect(await readdir(dir)).toEqual(['ink.toml'])
  })
})

describe('replacing a preset', () => {
  it('rewrites in place and reads back what it wrote', async () => {
    const path = join(dir, 'ink.toml')
    await writeFile(path, CURATED, 'utf8')
    await writePresetAtomically(path, 'name = "ink"\n')
    expect((await readPreset(path)).text).toBe('name = "ink"\n')
    // The temporary file is renamed, never left: it is invisible to the
    // watcher's glob but not to a person looking at their preset directory.
    expect(await readdir(dir)).toEqual(['ink.toml'])
  })
})
