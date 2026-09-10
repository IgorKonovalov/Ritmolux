/**
 * Main writes only where the player said it is looking (Plan 0159 Phase 6).
 *
 * The renderer hands a path back and main opens it, so without this guard main
 * is a general-purpose file writer that a compromised renderer drives over the
 * whole disk. The scope is the player's own answer — the file on screen and the
 * directory the `roster` event named — which is the same rule ADR-0184 states
 * for why the studio resolves nothing itself.
 */
import { resolve } from 'node:path'

import { describe, expect, it } from 'vitest'

import { isInScope, type PresetScope } from './presetHandlers'

const DIR = resolve('/presets')
const SCOPE: PresetScope = { file: resolve('/elsewhere/pinned.toml'), dir: DIR }

describe('a path main will open', () => {
  it('accepts a preset inside the directory the player named', () => {
    expect(isInScope(resolve(DIR, 'aurora.toml'), SCOPE)).toBe(true)
  })

  it('accepts the file on screen even when it is outside that directory', () => {
    // `--preset` can pin a file the watcher's directory does not contain.
    expect(isInScope(resolve('/elsewhere/pinned.toml'), SCOPE)).toBe(true)
  })

  it('refuses a sibling directory whose name merely starts the same', () => {
    expect(isInScope(resolve('/presets-backup/aurora.toml'), SCOPE)).toBe(false)
  })

  it('refuses a walk back out of the directory', () => {
    expect(isInScope(resolve(DIR, '..', 'secrets', 'id.toml'), SCOPE)).toBe(false)
    expect(isInScope(`${DIR}/../secrets/id.toml`, SCOPE)).toBe(false)
  })

  it('refuses anything that is not a preset file', () => {
    expect(isInScope(resolve(DIR, 'notes.txt'), SCOPE)).toBe(false)
    expect(isInScope(resolve(DIR, '.git', 'config'), SCOPE)).toBe(false)
  })

  it('refuses everything when the player has named nothing yet', () => {
    expect(isInScope(resolve(DIR, 'aurora.toml'), { file: undefined, dir: undefined })).toBe(false)
  })
})
