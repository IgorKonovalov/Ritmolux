/**
 * A settings file the user hand-edited must not stop the studio opening
 * (Plan 0159 Phase 1).
 */
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { readSettings, settingsFile } from './settings'

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

  it('puts the file in the per-user directory it was given', () => {
    expect(settingsFile('/users/vj/AppData/ritmolux-studio')).toBe(
      join('/users/vj/AppData/ritmolux-studio', 'settings.json'),
    )
  })
})
