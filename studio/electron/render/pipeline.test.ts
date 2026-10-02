/**
 * The render job's wiring and its verdicts, with stub children (Plan 0247
 * Phase 2): Cancel stops every child and removes the output, a stage that
 * fails is the stage named, and the encoder reads the player's own pipe.
 */
import { EventEmitter } from 'node:events'
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { PassThrough } from 'node:stream'
import type { ChildProcess, SpawnOptions } from 'node:child_process'

import { describe, expect, it, vi } from 'vitest'

import { RenderJob, type JobSpec, type PipelineStage } from './pipeline'

class StubChild extends EventEmitter {
  readonly stdout = new PassThrough()
  readonly stderr = new PassThrough()
  readonly progress = new PassThrough()
  readonly stdio: unknown[]
  readonly kill = vi.fn((signal?: string) => {
    setImmediate(() => this.exit(null, signal ?? 'SIGTERM'))
    return true
  })
  private exited = false

  constructor(readonly options: SpawnOptions) {
    super()
    this.stdio = [null, this.stdout, this.stderr, this.progress]
  }

  exit(code: number | null, signal: string | null = null): void {
    if (this.exited) return
    this.exited = true
    this.stdout.end()
    this.stderr.end()
    this.progress.end()
    this.emit('exit', code, signal)
    this.emit('close', code, signal)
  }
}

function harness(stages: PipelineStage[] = ['player', 'encoder']) {
  const dir = mkdtempSync(join(tmpdir(), 'rlx-render-'))
  const output = join(dir, 'clip.mp4')
  writeFileSync(output, 'a partial file')
  const children: StubChild[] = []
  const spawner = vi.fn((_command: string, _args: readonly string[], options: SpawnOptions) => {
    const child = new StubChild(options)
    children.push(child)
    return child as unknown as ChildProcess
  })
  const spec: JobSpec = {
    stages: stages.map((stage) => ({ stage, command: `/bin/${stage}`, args: ['--go'] })),
    output,
    log: `${output}.render.log`,
  }
  const job = new RenderJob(spec, {}, spawner)
  return { job, children, spawner, output, log: spec.log }
}

/** Let the stub children's queued exits and stream ends run. */
const settle = () => new Promise((resolve) => setImmediate(resolve))

describe('the pipeline wiring', () => {
  it('spawns the encoder with the player stdout as its stdin stream, not a pipe JavaScript forwards', () => {
    const { job, children, spawner } = harness()
    void job.start()
    const playerOptions = spawner.mock.calls[0][2]
    const encoderOptions = spawner.mock.calls[1][2]
    expect((playerOptions.stdio as unknown[])[0]).toBe('ignore')
    expect((playerOptions.stdio as unknown[])[1]).toBe('pipe')
    const stdin = (encoderOptions.stdio as unknown[])[0]
    expect(stdin).toBe(children[0].stdout)
    expect(stdin).not.toBe('pipe')
    // Progress arrives on descriptor 3 and the encoder's stdout goes nowhere.
    expect(encoderOptions.stdio).toEqual([children[0].stdout, 'ignore', 'pipe', 'pipe'])
    // The parent let go of its copy, so nothing here reads a frame.
    expect(children[0].stdout.destroyed).toBe(true)
    job.cancel()
  })

  it('chains a middle stage the same way', () => {
    const { job, children, spawner } = harness(['player', 'sidecar', 'encoder'])
    void job.start()
    expect((spawner.mock.calls[1][2].stdio as unknown[])[0]).toBe(children[0].stdout)
    expect((spawner.mock.calls[2][2].stdio as unknown[])[0]).toBe(children[1].stdout)
    job.cancel()
  })
})

describe('cancel', () => {
  it('kills every child and removes the output file', async () => {
    const { job, children, output } = harness(['player', 'sidecar', 'encoder'])
    const outcome = job.start()
    job.cancel()
    for (const child of children) expect(child.kill).toHaveBeenCalled()
    expect(await outcome).toEqual({ kind: 'cancelled' })
    expect(existsSync(output)).toBe(false)
  })
})

describe('a stage that fails', () => {
  it.each([
    [['player', 'encoder'], 0, 'player'],
    [['player', 'encoder'], 1, 'encoder'],
    [['player', 'sidecar', 'encoder'], 1, 'sidecar'],
  ] as const)('in %j at %i fails the job naming %s', async (stages, failing, name) => {
    const { job, children, output, log } = harness([...stages])
    const outcome = job.start()
    children[failing].stderr.write('something went wrong\n')
    await settle()
    children[failing].exit(1)
    const result = await outcome
    expect(result.kind).toBe('failed')
    if (result.kind !== 'failed') return
    expect(result.stage).toBe(name)
    expect(result.reason).toContain('code 1')
    expect(result.reason).toContain('something went wrong')
    expect(result.tail).toEqual(['something went wrong'])
    // The others were stopped, not blamed.
    for (const [index, child] of children.entries()) {
      if (index !== failing) expect(child.kill).toHaveBeenCalled()
    }
    expect(existsSync(output)).toBe(false)
    expect(result.log).toBe(log)
    expect(readFileSync(log, 'utf8')).toContain(`== ${name}: /bin/${name} --go`)
  })

  it('names the player when it dies first and the encoder then exits 0 on a short stream', async () => {
    const { job, children } = harness()
    const outcome = job.start()
    children[0].exit(1)
    children[1].progress.write('frame=3\nprogress=end\n')
    await settle()
    children[1].exit(0)
    const result = await outcome
    expect(result.kind === 'failed' && result.stage).toBe('player')
  })

  it('does not blame the player for a broken pipe after the encoder finished cleanly', async () => {
    const { job, children } = harness()
    const outcome = job.start()
    children[1].progress.write('frame=60\nprogress=end\n')
    await settle()
    children[1].exit(0)
    await settle()
    children[0].exit(1)
    expect(await outcome).toEqual({ kind: 'done' })
  })

  it('fails an encoder that exits 0 without reporting the end of its stream', async () => {
    const { job, children } = harness()
    const outcome = job.start()
    children[0].exit(0)
    children[1].exit(0)
    const result = await outcome
    expect(result.kind === 'failed' && result.stage).toBe('encoder')
  })

  it('fails a stage that could not be spawned, naming it', async () => {
    const { job, children } = harness()
    const outcome = job.start()
    children[1].emit('error', new Error('spawn ffmpeg ENOENT'))
    const result = await outcome
    expect(result.kind === 'failed' && result.stage).toBe('encoder')
    expect(result.kind === 'failed' && result.reason).toContain('ENOENT')
  })
})

describe('progress', () => {
  it('reports the encoder frame count as it moves, and finishes done', async () => {
    const dir = mkdtempSync(join(tmpdir(), 'rlx-render-'))
    const frames: number[] = []
    const children: StubChild[] = []
    const job = new RenderJob(
      {
        stages: [
          { stage: 'player', command: 'p', args: [] },
          { stage: 'encoder', command: 'e', args: [] },
        ],
        output: join(dir, 'clip.mp4'),
        log: join(dir, 'clip.mp4.render.log'),
      },
      { onProgress: (frame) => frames.push(frame) },
      (_c, _a, options) => {
        const child = new StubChild(options)
        children.push(child)
        return child as unknown as ChildProcess
      },
    )
    const outcome = job.start()
    children[1].progress.write('frame=10\nprogress=continue\nframe=10\nprogress=continue\n')
    children[1].progress.write('frame=25\nprogress=end\n')
    await settle()
    children[0].exit(0)
    children[1].exit(0)
    expect(await outcome).toEqual({ kind: 'done' })
    expect(frames).toEqual([10, 25])
  })
})
