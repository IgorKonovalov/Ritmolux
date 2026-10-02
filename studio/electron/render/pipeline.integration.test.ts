/**
 * The real three-stage pipeline (Plan 0247 Phase 4): a built player's
 * `--render`, `sd_filter.py --passthrough` and `ffmpeg`, joined the way a
 * neural job joins them, over a two-second WAV the test writes.
 *
 * It **skips with a printed notice** (ADR-0016's shape) when any of the three
 * is missing, when the built player predates `--bars`, or when the machine has
 * no GPU adapter to render with.
 *
 * `--passthrough` parses the stream and loads no model, and it does not read
 * `--timeline` either, so the files written beside the output are also loaded
 * through the sidecar's own `load_timeline` and `load_bar_grid` here.
 */
import { execFile, spawn, spawnSync, type SpawnOptions } from 'node:child_process'
import { mkdirSync, mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { builtPlayer } from '../testing/player'
import { encoderArgs, neuralFiles, playerRenderArgs, readBars, writeTimeline } from './commands'
import { RenderJob } from './pipeline'
import type { RunTool } from './transcode'

const SCRIPT = join(__dirname, '..', '..', '..', 'tools', 'sd-filter', 'sd_filter.py')
const PYTHON = process.platform === 'win32' ? 'python' : 'python3'

/** A one-preset library, bound to the audio. */
const PRESET = 'system = "fragment_field"\nname = "Render Probe"\n[params]\nzoom = "1.1 + 0.6 * bass"\n'

/** Whether `command` runs at all, asked with `args`. */
function runs(command: string, args: string[]): boolean {
  const result = spawnSync(command, args, { stdio: 'ignore', windowsHide: true })
  return result.error === undefined && result.status === 0
}

/** Two seconds of stereo 48 kHz: a kick every half second over a quiet tone. */
function writeClip(path: string): void {
  const rate = 48_000
  const frames = 2 * rate
  const bytes = Buffer.alloc(44 + frames * 4)
  bytes.write('RIFF', 0)
  bytes.writeUInt32LE(bytes.length - 8, 4)
  bytes.write('WAVEfmt ', 8)
  bytes.writeUInt32LE(16, 16)
  bytes.writeUInt16LE(1, 20)
  bytes.writeUInt16LE(2, 22)
  bytes.writeUInt32LE(rate, 24)
  bytes.writeUInt32LE(rate * 4, 28)
  bytes.writeUInt16LE(4, 32)
  bytes.writeUInt16LE(16, 34)
  bytes.write('data', 36)
  bytes.writeUInt32LE(frames * 4, 40)
  for (let frame = 0; frame < frames; frame += 1) {
    const t = frame / rate
    const sinceKick = t % 0.5
    const kick = Math.exp(-sinceKick * 18) * Math.sin(2 * Math.PI * 55 * sinceKick)
    const tone = 0.1 * Math.sin(2 * Math.PI * 440 * t)
    const sample = Math.round(Math.max(-1, Math.min(1, 0.8 * kick + tone)) * 32767)
    bytes.writeInt16LE(sample, 44 + frame * 4)
    bytes.writeInt16LE(sample, 46 + frame * 4)
  }
  writeFileSync(path, bytes)
}

/** Frames in the first video stream of `file`, decoded and counted by ffmpeg. */
function frameCount(file: string): number {
  const result = spawnSync(
    'ffmpeg',
    ['-v', 'error', '-i', file, '-map', '0:v:0', '-f', 'null', '-progress', 'pipe:1', '-'],
    { encoding: 'utf8', windowsHide: true },
  )
  const counts = [...result.stdout.matchAll(/^frame=(\d+)$/gm)].map((match) => Number(match[1]))
  return counts.at(-1) ?? -1
}

describe('the three-stage render pipeline', () => {
  it('writes an MP4 with exactly as many frames as --bars counted', async () => {
    const player = builtPlayer()
    if (player.path === undefined) {
      console.warn(`skipped: ${player.missing}`)
      return
    }
    if (!runs('ffmpeg', ['-version'])) {
      console.warn('skipped: no ffmpeg on PATH')
      return
    }
    if (!runs(PYTHON, ['--version'])) {
      console.warn(`skipped: no ${PYTHON} on PATH`)
      return
    }
    const help = spawnSync(player.path, ['--help'], { encoding: 'utf8', windowsHide: true })
    if (!`${help.stdout}${help.stderr}`.includes('--bars')) {
      console.warn(`skipped: ${player.path} predates --bars; rebuild it`)
      return
    }

    const dir = mkdtempSync(join(tmpdir(), 'rlx-pipeline-'))
    const presets = join(dir, 'presets')
    const root = join(dir, 'player-data')
    mkdirSync(presets)
    mkdirSync(root)
    writeFileSync(join(presets, 'probe.toml'), PRESET)
    // The player reads this library and keeps its data under the scratch root.
    const env = { ...process.env, RLX_PRESET_DIR: presets, APPDATA: root, HOME: root, XDG_DATA_HOME: root }
    const run: RunTool = (command, args) =>
      new Promise((resolve, reject) => {
        execFile(command, args, { env, windowsHide: true }, (error, stdout, stderr) =>
          error ? reject(new Error(`${error.message}\n${stderr}`)) : resolve(stdout),
        )
      })

    const wav = join(dir, 'clip.wav')
    writeClip(wav)
    const output = join(dir, 'clip.mp4')
    const files = neuralFiles(output)
    const grid = await readBars(run, player.path, wav, '30', files.bars)
    writeTimeline(files.timeline, [{ at_bar: 1, prompt: 'a vast canyon of luminous rock' }])

    // The two files beside the output, through the sidecar's own loaders.
    const loaded = spawnSync(
      PYTHON,
      [
        '-c',
        'import sys; sys.path.insert(0, sys.argv[1]); import sd_filter; ' +
          'sd_filter.load_timeline(sys.argv[2]); sd_filter.load_bar_grid(sys.argv[3])',
        dirname(SCRIPT),
        files.timeline,
        files.bars,
      ],
      { encoding: 'utf8', windowsHide: true },
    )
    expect(loaded.status, `the sidecar refused the files:\n${loaded.stderr}`).toBe(0)

    const request = {
      source: wav,
      preset: 'Render Probe',
      fps: '30',
      size: '160x90',
      tier: 'floor' as const,
      output,
      neural: null,
    }
    const job = new RenderJob(
      {
        stages: [
          { stage: 'player', command: player.path, args: playerRenderArgs(wav, request) },
          {
            stage: 'sidecar',
            command: PYTHON,
            args: [SCRIPT, '--passthrough', '--timeline', files.timeline, '--bar-grid', files.bars],
          },
          { stage: 'encoder', command: 'ffmpeg', args: encoderArgs(wav, output) },
        ],
        output,
        log: `${output}.render.log`,
      },
      {},
      (command, args, options: SpawnOptions) => spawn(command, args, { ...options, env }),
    )
    const outcome = await job.start()
    if (
      outcome.kind === 'failed' &&
      outcome.stage === 'player' &&
      outcome.tail.join('\n').includes('no suitable GPU adapter')
    ) {
      console.warn('skipped: no GPU adapter on this machine (ADR-0016)')
      return
    }
    expect(outcome, JSON.stringify(outcome)).toEqual({ kind: 'done' })
    expect(job.frame).toBe(grid.frames)
    expect(frameCount(output)).toBe(grid.frames)
  }, 300_000)
})
