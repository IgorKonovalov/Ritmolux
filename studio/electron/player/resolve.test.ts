/**
 * The player is found in ADR-0178's order: bundled, settings, PATH
 * (Plan 0159 Phase 1).
 */
import { delimiter, join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { playerCandidates, playerExecutableName, resolvePlayer } from './resolve'

// Built with the host's own separators: the function under test joins and
// splits with this platform's rules, so an expectation written in POSIX would
// be asserting the test's syntax rather than the resolution order.
const PATH_DIRS = [join('/usr', 'local', 'bin'), join('/usr', 'bin')]
const inputs = {
  resourcesPath: join('/app', 'resources'),
  settingsPath: join('/home', 'vj', 'builds', 'ritmolux'),
  pathEnv: PATH_DIRS.join(delimiter),
  platform: 'darwin' as const,
}

describe('player resolution', () => {
  it('names the executable per platform', () => {
    expect(playerExecutableName('win32')).toBe('ritmolux.exe')
    expect(playerExecutableName('darwin')).toBe('ritmolux')
  })

  it('searches bundled, then settings, then PATH, in that order', () => {
    const order = playerCandidates({ ...inputs, exists: () => false }).map((c) => c.source)
    expect(order).toEqual(['bundled', 'settings', 'path', 'path'])
  })

  it('prefers the bundled copy, so a tester configures nothing', () => {
    const found = resolvePlayer({ ...inputs, exists: () => true })
    expect(found).toEqual({
      path: join(inputs.resourcesPath, 'player', 'ritmolux'),
      source: 'bundled',
    })
  })

  it('falls back to the settings path when nothing is bundled', () => {
    const found = resolvePlayer({
      ...inputs,
      resourcesPath: undefined,
      exists: (candidate) => candidate === inputs.settingsPath,
    })
    expect(found).toEqual({ path: inputs.settingsPath, source: 'settings' })
  })

  it('falls back to PATH last, taking the first directory that has one', () => {
    const found = resolvePlayer({
      ...inputs,
      resourcesPath: undefined,
      settingsPath: undefined,
      exists: (candidate) => candidate.startsWith(PATH_DIRS[0]),
    })
    expect(found).toEqual({ path: join(PATH_DIRS[0], 'ritmolux'), source: 'path' })
  })

  it('reports nothing rather than a path that does not exist', () => {
    expect(resolvePlayer({ ...inputs, exists: () => false })).toBeUndefined()
  })

  it('ignores an empty settings path rather than searching the current directory', () => {
    const candidates = playerCandidates({ ...inputs, settingsPath: '', exists: () => false })
    expect(candidates.some((c) => c.source === 'settings')).toBe(false)
  })
})
