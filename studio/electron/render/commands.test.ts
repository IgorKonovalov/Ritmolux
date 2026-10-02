/**
 * The render's command lines (Plan 0247 Phase 2): the encode is `shot`'s
 * canonical one, read off the Rust source, and `--bars` is read through the
 * grid validator.
 */
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import {
  encoderArgs,
  ENCODER_CRF,
  neuralFiles,
  playerRenderArgs,
  readBars,
  sidecarArgs,
} from './commands'

const RENDER_RS = join(__dirname, '..', '..', '..', 'standalone', 'src', 'shot', 'render.rs')

/** Every string literal in the body of `pub fn ffmpeg_args`, in order. */
function canonicalLiterals(): string[] {
  const source = readFileSync(RENDER_RS, 'utf8')
  const start = source.indexOf('pub fn ffmpeg_args(')
  if (start === -1) throw new Error('render.rs no longer declares ffmpeg_args')
  const end = source.indexOf('\n}\n', start)
  const body = source.slice(start, end)
  const literals = [...body.matchAll(/"([^"]*)"/g)].map((match) => match[1])
  if (literals.length < 20) throw new Error(`read only ${literals.length} literals from ffmpeg_args`)
  return literals
}

describe('the encode', () => {
  it("is shot's canonical command line, plus the progress channel", () => {
    const args = encoderArgs('/cache/track.wav', '/videos/clip.mp4')
    const literals = canonicalLiterals()
    // Walk the literals through the list in order; what is left over is what
    // the Rust function fills from its parameters, and the progress flag.
    const rest: string[] = []
    let next = 0
    for (const arg of args) {
      if (next < literals.length && arg === literals[next]) next += 1
      else rest.push(arg)
    }
    expect(next).toBe(literals.length)
    expect(rest).toEqual([
      '/cache/track.wav',
      String(ENCODER_CRF),
      '-progress',
      'pipe:3',
      '/videos/clip.mp4',
    ])
    expect(args.at(-1)).toBe('/videos/clip.mp4')
  })

  it('asks the player for the request it was given', () => {
    expect(
      playerRenderArgs('/cache/t.wav', {
        source: '/m/t.flac',
        preset: 'Lace Grid',
        fps: '30',
        size: '1920x1080',
        tier: 'rich',
        output: '/v/c.mp4',
        neural: null,
      }),
    ).toEqual([
      '--render',
      '/cache/t.wav',
      '--preset',
      'Lace Grid',
      '--fps',
      '30',
      '--size',
      '1920x1080',
      '--tier',
      'rich',
    ])
  })
})

describe('the sidecar', () => {
  it('reads the two files beside the output, and passes negative and seed only when set', () => {
    const files = neuralFiles('/v/c.mp4')
    expect(files).toEqual({ bars: '/v/c.mp4.bars.json', timeline: '/v/c.mp4.timeline.json' })
    const timeline = [{ at_bar: 1, prompt: 'a canyon' }]
    expect(sidecarArgs('/src/sd_filter.py', { profile: 'quality', negative: null, seed: null, timeline }, files)).toEqual([
      '/src/sd_filter.py',
      '--profile',
      'quality',
      '--timeline',
      '/v/c.mp4.timeline.json',
      '--bar-grid',
      '/v/c.mp4.bars.json',
    ])
    expect(
      sidecarArgs('/s.py', { profile: 'fast', negative: 'blurry', seed: 0, timeline }, files).slice(-4),
    ).toEqual(['--negative', 'blurry', '--seed', '0'])
  })
})

describe('the bars call', () => {
  it('runs --bars and reads the grid it wrote', async () => {
    const out = join(mkdtempSync(join(tmpdir(), 'rlx-bars-')), 'g.json')
    const calls: string[][] = []
    const grid = await readBars(
      (command, args) => {
        calls.push([command, ...args])
        writeFileSync(out, '{"fps":"30","frames":120,"bar_starts":[0,48,96],"bar_locked":[false,true,false]}\n')
        return Promise.resolve('')
      },
      '/bin/ritmolux',
      '/cache/t.wav',
      '30',
      out,
    )
    expect(calls).toEqual([['/bin/ritmolux', '--bars', '/cache/t.wav', '--fps', '30', '--out', out]])
    expect(grid.frames).toBe(120)
    expect(grid.bar_locked).toEqual([false, true, false])
  })

  it('names the player when it refused', async () => {
    await expect(
      readBars(() => Promise.reject(new Error('--bars x.wav: not 16-bit PCM')), 'p', 'x.wav', '30', 'o'),
    ).rejects.toThrow(/could not count the bars: --bars x.wav/)
  })
})
