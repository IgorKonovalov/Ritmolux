/**
 * A settings file the user hand-edited must not stop the studio opening
 * (Plan 0159 Phase 1), and the player mode it carries survives a restart
 * (Plan 0167 Phase 5).
 */
import { existsSync, mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { DEFAULT_PLAYER_MODE } from '@shared/player-mode'

import { playerModeOf, readSettings, settingsFile, writeSettings } from './settings'

function withContent(content: string): string {
  const file = join(mkdtempSync(join(tmpdir(), 'rlx-studio-')), 'settings.json')
  writeFileSync(file, content)
  return file
}

describe('readSettings', () => {
  it('reads a player path', () => {
    expect(readSettings(withContent('{"playerPath":"/opt/ritmolux"}'))).toEqual({
      playerPath: '/opt/ritmolux',
    })
  })

  it('degrades to no setting when the file is absent', () => {
    expect(readSettings(join(tmpdir(), 'rlx-studio-nothing-here', 'settings.json'))).toEqual({})
  })

  it('degrades to no setting on a trailing comma rather than failing the launch', () => {
    expect(readSettings(withContent('{"playerPath":"/opt/ritmolux",}'))).toEqual({})
  })

  it('degrades to no setting when playerPath is not a string', () => {
    expect(readSettings(withContent('{"playerPath":42}'))).toEqual({})
    expect(readSettings(withContent('[]'))).toEqual({})
    expect(readSettings(withContent('null'))).toEqual({})
  })

  it('reads a player mode the studio knows', () => {
    expect(readSettings(withContent('{"playerMode":"windowless"}'))).toEqual({
      playerMode: 'windowless',
    })
  })

  it('drops a mode it does not know rather than refusing the whole file', () => {
    // The rest of the file is still usable and the default is a working answer,
    // so a hand-typed `"windowles"` costs the mode and not the player path.
    expect(
      readSettings(withContent('{"playerPath":"/opt/ritmolux","playerMode":"windowles"}')),
    ).toEqual({ playerPath: '/opt/ritmolux' })
  })

  it('puts the file in the per-user directory it was given', () => {
    expect(settingsFile('/users/vj/AppData/ritmolux-studio')).toBe(
      join('/users/vj/AppData/ritmolux-studio', 'settings.json'),
    )
  })
})

describe('the mode the studio spawns with', () => {
  it('is windowed when nothing was ever chosen', () => {
    // The VJ with a projector is who the player is for (ADR-0186).
    expect(playerModeOf({})).toBe('windowed')
    expect(playerModeOf(readSettings(withContent('{}')))).toBe(DEFAULT_PLAYER_MODE)
  })

  it('is windowed when the file says something unrecognised', () => {
    expect(playerModeOf(readSettings(withContent('{"playerMode":42}')))).toBe('windowed')
  })

  it('survives a restart, which is the whole reason it is a setting', () => {
    const file = withContent('{"playerPath":"/opt/ritmolux"}')
    const before = readSettings(file)
    writeSettings(file, { ...before, playerMode: 'windowless' })

    const after = readSettings(file)
    expect(playerModeOf(after)).toBe('windowless')
    // Merged, not replaced: losing the player path here is a studio that finds
    // no player on the next launch, and nothing would say why.
    expect(after.playerPath).toBe('/opt/ritmolux')
  })

  it('leaves no temporary file behind, because the write is a rename', () => {
    const file = withContent('{}')
    writeSettings(file, { playerMode: 'windowless' })
    expect(existsSync(`${file}.tmp`)).toBe(false)
  })

  it('writes a file the studio can read on a directory that does not exist yet', () => {
    // The per-user directory is created by Electron, but a settings write is the
    // first thing that touches it on a machine where nothing else has.
    const file = join(mkdtempSync(join(tmpdir(), 'rlx-studio-')), 'nested', 'settings.json')
    writeSettings(file, { playerMode: 'windowless' })
    expect(playerModeOf(readSettings(file))).toBe('windowless')
  })
})
