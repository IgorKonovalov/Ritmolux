/**
 * Main renders only what it was shown, and one job at a time (Plan 0247
 * Phase 2).
 */
import { EventEmitter } from 'node:events'
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { PassThrough } from 'node:stream'
import type { ChildProcess } from 'node:child_process'

import { describe, expect, it, vi } from 'vitest'

import type { RenderEvent } from '@shared/render'

import { RenderService, SidecarMeter, suggestedName } from './service'
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
  const script = join(dir, 'sd_filter.py')
  writeFileSync(script, '# the sidecar')
  let cuda = 'True'
  const run: RunTool = vi.fn((_command: string, args: string[]) => {
    if (args[0] === '-c') return Promise.resolve(`${cuda}\n`)
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
    diffusion: () => ({ python: '/venv/bin/python', script }),
    outputDir: () => join(dir, 'Videos'),
    cache: new TranscodeCache(join(dir, 'cache'), () => '/bin/ffmpeg', run),
    emit: (event) => events.push(event),
    stayAwake: () => release,
    run,
    spawn,
  })
  return { service, source, spawned, events, release, script, noCuda: () => (cuda = 'False') }
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
      neural: null,
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
    const request = { source, preset: 'Gyre', fps: '30', size: '1920x1080', tier: 'rich', output: output.value, neural: null }
    expect(await service.start(request)).toEqual({ ok: true, value: null })
    expect(spawned.map((s) => s.command)).toEqual(['/bin/ritmolux', '/bin/ffmpeg'])
    expect(spawned[0].args.slice(0, 1)).toEqual(['--render'])
    expect(spawned[1].args.at(-1)).toBe(output.value)
    expect(events[0]).toEqual({ kind: 'started', output: output.value, frames: 90 })
    expect(service.busy).toBe(true)
    expect(await service.start(request)).toEqual({ ok: false, reason: 'a render is already running' })
  })
})

describe('a neural job', () => {
  function neuralRequest(source: string, output: string, timeline: { at_bar: number; prompt: string }[]) {
    return {
      source,
      preset: 'Gyre',
      fps: '30',
      size: '1920x1080',
      tier: 'rich',
      output,
      neural: { profile: 'fast', negative: 'blurry', seed: 7, timeline },
    }
  }

  it('is refused with no prompt before anything is spawned or transcoded', async () => {
    const { service, source, spawned } = harness()
    service.grant(source)
    const output = service.suggestOutput(source, 'Gyre')
    if (!output.ok) throw new Error(output.reason)
    for (const timeline of [[], [{ at_bar: 1, prompt: '  ' }]]) {
      const result = await service.start(neuralRequest(source, output.value, timeline))
      expect(result).toMatchObject({ ok: false, reason: expect.stringContaining('cannot start') })
    }
    expect(spawned).toEqual([])
    expect(existsSync(`${output.value}.timeline.json`)).toBe(false)
  })

  it('is refused while the probe says no, with its reason', async () => {
    const { service, source, spawned, noCuda } = harness()
    noCuda()
    service.grant(source)
    const output = service.suggestOutput(source, 'Gyre')
    if (!output.ok) throw new Error(output.reason)
    const result = await service.start(neuralRequest(source, output.value, [{ at_bar: 1, prompt: 'a canyon' }]))
    expect(result).toMatchObject({ ok: false, reason: expect.stringContaining('torch reports no CUDA') })
    expect(spawned).toEqual([])
  })

  it('writes the grid and the timeline beside the output and splices the sidecar in', async () => {
    const { service, source, spawned, script } = harness()
    service.grant(source)
    const output = service.suggestOutput(source, 'Gyre')
    if (!output.ok) throw new Error(output.reason)
    const timeline = [
      { at_bar: 1, prompt: 'a vast canyon' },
      { at_bar: 2, prompt: 'a rose window' },
    ]
    expect(await service.start(neuralRequest(source, output.value, timeline))).toEqual({ ok: true, value: null })

    expect(JSON.parse(readFileSync(`${output.value}.timeline.json`, 'utf8'))).toEqual(timeline)
    expect(readFileSync(`${output.value}.bars.json`, 'utf8')).toBe(GRID)
    expect(spawned.map((s) => s.command)).toEqual(['/bin/ritmolux', '/venv/bin/python', '/bin/ffmpeg'])
    expect(spawned[1].args).toEqual([
      script,
      '--profile',
      'fast',
      '--timeline',
      `${output.value}.timeline.json`,
      '--bar-grid',
      `${output.value}.bars.json`,
      '--negative',
      'blurry',
      '--seed',
      '7',
    ])
  })

  it('refuses a prompt past the grid the player counted', async () => {
    const { service, source, spawned } = harness()
    service.grant(source)
    const output = service.suggestOutput(source, 'Gyre')
    if (!output.ok) throw new Error(output.reason)
    const result = await service.start(neuralRequest(source, output.value, [{ at_bar: 3, prompt: 'late' }]))
    expect(result).toMatchObject({ ok: false, reason: expect.stringContaining('past the track') })
    expect(spawned).toEqual([])
  })
})

describe('the sidecar meter', () => {
  it('counts from its lines and paces from the first one', () => {
    let now = 0
    const meter = new SidecarMeter(() => now)
    expect(meter.read('sd-filter: stream 1920x1080 C444, 6220800 bytes/frame')).toBe(false)
    expect(meter.reading()).toEqual({ frames: 0, secondsPerFrame: undefined })
    now = 60_000
    expect(meter.read('sd-filter: 10 frames')).toBe(true)
    expect(meter.reading()).toEqual({ frames: 10, secondsPerFrame: undefined })
    now = 100_000
    meter.read('sd-filter: 20 frames')
    expect(meter.reading()).toEqual({ frames: 20, secondsPerFrame: 4 })
  })
})
