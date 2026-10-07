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

import {
  ffmpegOf,
  outputDirOf,
  playerModeOf,
  readSettings,
  reducedMotionOf,
  settingsFile,
  sourceDirOf,
  withJudgingSource,
  withRender,
  writeSettings,
} from './settings'

describe('judging.sourceDir', () => {
  it('reads the key and drops one of the wrong shape', () => {
    expect(readSettings(withContent('{"judging":{"sourceDir":"/w/presets"}}'))).toEqual({
      judging: { sourceDir: '/w/presets' },
    })
    expect(readSettings(withContent('{"judging":{"sourceDir":3},"playerPath":"/p"}'))).toEqual({
      playerPath: '/p',
    })
  })

  it('is set and cleared without touching the other keys', () => {
    const set = withJudgingSource({ playerPath: '/p' }, '/w/presets')
    expect(set).toEqual({ playerPath: '/p', judging: { sourceDir: '/w/presets' } })
    expect(sourceDirOf(set)).toBe('/w/presets')
    expect(withJudgingSource(set, null)).toEqual({ playerPath: '/p' })
    expect(withJudgingSource(set, '  ')).toEqual({ playerPath: '/p' })
    expect(sourceDirOf({})).toBeUndefined()
  })
})

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

  it('reads ui.reducedMotion, and drops it when it is not a boolean', () => {
    expect(readSettings(withContent('{"ui":{"reducedMotion":true}}'))).toEqual({
      ui: { reducedMotion: true },
    })
    expect(readSettings(withContent('{"playerMode":"windowless","ui":{"reducedMotion":"yes"}}'))).toEqual({
      playerMode: 'windowless',
    })
    expect(readSettings(withContent('{"ui":[true]}'))).toEqual({})
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

describe('the render keys', () => {
  it('read ffmpegPath and outputDir, and default both when absent', () => {
    const settings = readSettings(
      withContent('{"render":{"ffmpegPath":"/opt/ffmpeg/bin/ffmpeg","outputDir":"/data/clips"}}'),
    )
    expect(settings.render).toEqual({
      ffmpegPath: '/opt/ffmpeg/bin/ffmpeg',
      outputDir: '/data/clips',
    })
    expect(ffmpegOf(settings)).toBe('/opt/ffmpeg/bin/ffmpeg')
    expect(outputDirOf(settings, '/home/vj/Videos')).toBe('/data/clips')

    expect(ffmpegOf({})).toBe('ffmpeg')
    expect(outputDirOf({}, '/home/vj/Videos')).toBe('/home/vj/Videos')
  })

  it('drop a key of the wrong shape and keep the other', () => {
    expect(readSettings(withContent('{"render":{"ffmpegPath":7,"outputDir":"/clips"}}'))).toEqual({
      render: { outputDir: '/clips' },
    })
    expect(readSettings(withContent('{"render":{"ffmpegPath":""}}'))).toEqual({})
    expect(readSettings(withContent('{"render":"ffmpeg"}'))).toEqual({})
  })

  it('survive a restart beside the other keys, and an empty value clears one', () => {
    const file = withContent('{"playerPath":"/opt/ritmolux","ui":{"reducedMotion":true}}')
    writeSettings(file, withRender(readSettings(file), { ffmpegPath: '/opt/ffmpeg', outputDir: '/clips' }))

    const after = readSettings(file)
    expect(after.render).toEqual({ ffmpegPath: '/opt/ffmpeg', outputDir: '/clips' })
    expect(after.playerPath).toBe('/opt/ritmolux')
    expect(reducedMotionOf(after)).toBe(true)

    writeSettings(file, withRender(after, { outputDir: '' }))
    expect(readSettings(file).render).toEqual({ ffmpegPath: '/opt/ffmpeg' })
    writeSettings(file, withRender(readSettings(file), { ffmpegPath: null }))
    expect(readSettings(file).render).toBeUndefined()
  })
})

describe('the diffusion keys', () => {
  it('are absent by default, and read key by key', () => {
    expect(readSettings(withContent('{}')).render).toBeUndefined()
    expect(
      readSettings(
        withContent('{"render":{"diffusion":{"python":"/venv/bin/python","script":7}}}'),
      ).render,
    ).toEqual({ diffusion: { python: '/venv/bin/python' } })
    expect(readSettings(withContent('{"render":{"diffusion":"yes"}}')).render).toBeUndefined()
  })

  it('round-trip beside the other render keys, and clear one at a time', () => {
    const file = withContent('{"render":{"ffmpegPath":"/opt/ffmpeg"}}')
    writeSettings(
      file,
      withRender(readSettings(file), {
        'diffusion.python': '/src/tools/sd-filter/.venv/bin/python',
        'diffusion.script': '/src/tools/sd-filter/sd_filter.py',
      }),
    )
    expect(readSettings(file).render).toEqual({
      ffmpegPath: '/opt/ffmpeg',
      diffusion: {
        python: '/src/tools/sd-filter/.venv/bin/python',
        script: '/src/tools/sd-filter/sd_filter.py',
      },
    })

    writeSettings(file, withRender(readSettings(file), { 'diffusion.python': null }))
    expect(readSettings(file).render).toEqual({
      ffmpegPath: '/opt/ffmpeg',
      diffusion: { script: '/src/tools/sd-filter/sd_filter.py' },
    })
    writeSettings(file, withRender(readSettings(file), { 'diffusion.script': '' }))
    expect(readSettings(file).render).toEqual({ ffmpegPath: '/opt/ffmpeg' })
  })
})

describe('whether the studio reduces its motion', () => {
  it('does not when nothing was ever chosen', () => {
    expect(reducedMotionOf({})).toBe(false)
    expect(reducedMotionOf(readSettings(withContent('{"ui":{}}')))).toBe(false)
  })

  it('survives a restart beside the other keys', () => {
    const file = withContent('{"playerPath":"/opt/ritmolux","playerMode":"windowless"}')
    const before = readSettings(file)
    writeSettings(file, { ...before, ui: { ...before.ui, reducedMotion: true } })

    const after = readSettings(file)
    expect(reducedMotionOf(after)).toBe(true)
    expect(after.playerPath).toBe('/opt/ritmolux')
    expect(playerModeOf(after)).toBe('windowless')
  })
})
