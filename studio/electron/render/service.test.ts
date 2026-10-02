/**
 * Main renders only what it was shown, and one job at a time (Plan 0247
 * Phase 2).
 */
import { EventEmitter } from 'node:events'
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { PassThrough } from 'node:stream'
import type { ChildProcess } from 'node:child_process'

import { describe, expect, it, vi } from 'vitest'

import type { RenderEvent } from '@shared/render'

import { RenderService, suggestedName } from './service'
import { TranscodeCache, type RunTool } from './transcode'

const GRID = '{"fps":"30","frames":90,"bar_starts":[0,40],"bar_locked":[false,true]}\n'

/** A mono 16-bit WAV of four silent samples: what the stub transcode writes. */
function silentWav(): Buffer {
  const bytes = Buffer.alloc(44 + 8)
  bytes.write('RIFF', 0)
  bytes.writeUInt32LE(bytes.length - 8, 4)
  bytes.write('WAVEfmt ', 8)
  bytes.writeUInt32LE(16, 16)
  bytes.writeUInt16LE(1, 20)
  bytes.writeUInt16LE(1, 22)
  bytes.writeUInt32LE(8000, 24)
  bytes.writeUInt32LE(16000, 28)
  bytes.writeUInt16LE(2, 32)
  bytes.writeUInt16LE(16, 34)
  bytes.write('data', 36)
  bytes.writeUInt32LE(8, 40)
  return bytes
}

function harness() {
  const dir = mkdtempSync(join(tmpdir(), 'rlx-service-'))
  const source = join(dir, 'track.flac')
  writeFileSync(source, 'flac')
  const run: RunTool = vi.fn((_command: string, args: string[]) => {
    // ffmpeg's transcode writes its last argument; --bars writes after --out.
    const out = args.includes('--out') ? args[args.indexOf('--out') + 1] : (args.at(-1) as string)
    writeFileSync(out, args.includes('--bars') ? GRID : silentWav())
    return Promise.resolve('')
  })
  const spawned: { command: string; args: readonly string[] }[] = []
  const spawn = vi.fn((command: string, args: readonly string[]) => {
    spawned.push({ command, args })
    const child = Object.assign(new EventEmitter(), {
      stdout: new PassThrough(),
      stderr: new PassThrough(),
      stdio: [null, new PassThrough(), new PassThrough(), new PassThrough()],
      kill: () => true,
    })
    return child as unknown as ChildProcess
  })
  const events: RenderEvent[] = []
  const release = vi.fn()
  const service = new RenderService({
    player: () => '/bin/ritmolux',
    ffmpeg: () => '/bin/ffmpeg',
    outputDir: () => join(dir, 'Videos'),
    cache: new TranscodeCache(join(dir, 'cache'), () => '/bin/ffmpeg', run),
    emit: (event) => events.push(event),
    stayAwake: () => release,
    run,
    spawn,
  })
  return { service, source, spawned, events, release }
}

describe('the suggested name', () => {
  it('is the track and the preset, in a file-safe spelling', () => {
    expect(suggestedName('/m/02 - Night Drive.flac', 'Lace Grid')).toBe('02-night-drive-lace-grid.mp4')
    expect(suggestedName('/m/???.mp3', '!!')).toBe('track-preset.mp4')
  })
})

describe('the render service', () => {
  it('refuses a track or an output it did not produce', async () => {
    const { service, source } = harness()
    expect(await service.prepare(source, '30')).toEqual({
      ok: false,
      reason: 'that track was not picked in this studio',
    })
    service.grant(source)
    const request = {
      source,
      preset: 'Gyre',
      fps: '30',
      size: '1920x1080',
      tier: 'rich',
      output: '/etc/passwd',
    }
    expect(await service.start(request)).toEqual({
      ok: false,
      reason: 'that output was not chosen in this studio',
    })
  })

  it('prepares a picked track into its bar grid', async () => {
    const { service, source } = harness()
    service.grant(source)
    const result = await service.prepare(source, '30')
    expect(result.ok && result.value.grid.bar_starts).toEqual([0, 40])
    expect(result.ok && result.value.peaks.max).toEqual([0, 0, 0, 0])
    expect(await service.prepare(source, '29.97')).toMatchObject({ ok: false })
  })

  it('starts the player into the encoder, holds the machine awake, and refuses a second job', async () => {
    const { service, source, spawned, events } = harness()
    service.grant(source)
    const output = service.suggestOutput(source, 'Gyre')
    expect(output.ok).toBe(true)
    if (!output.ok) return
    const request = { source, preset: 'Gyre', fps: '30', size: '1920x1080', tier: 'rich', output: output.value }
    expect(await service.start(request)).toEqual({ ok: true, value: null })
    expect(spawned.map((s) => s.command)).toEqual(['/bin/ritmolux', '/bin/ffmpeg'])
    expect(spawned[0].args.slice(0, 1)).toEqual(['--render'])
    expect(spawned[1].args.at(-1)).toBe(output.value)
    expect(events[0]).toEqual({ kind: 'started', output: output.value, frames: 90 })
    expect(service.busy).toBe(true)
    expect(await service.start(request)).toEqual({ ok: false, reason: 'a render is already running' })
  })
})
