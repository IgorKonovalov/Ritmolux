/**
 * One clip render: the player's `--render`, optionally the diffusion sidecar,
 * and `ffmpeg`, as children joined by OS pipes (ADR-0262).
 *
 * **No frame byte passes through this process.** Each child's `stdout` is
 * handed to the next child's spawn as its `stdin` stream, so Node duplicates the
 * pipe's descriptor into the child and the kernel connects the two. Node stops
 * reading that stream in the parent when it is handed over, which only holds
 * because the next child is spawned in the same tick as the one before it; the
 * parent then destroys its own copy, so a downstream child that dies leaves its
 * writer a broken pipe rather than a pipe nobody drains.
 *
 * What does cross is small: each child's standard error, kept as a bounded
 * tail, and the encoder's `-progress` stream on descriptor 3.
 *
 * **Which stage failed.** The first stage to exit badly is the one named, and
 * every other stage is then stopped. One exception holds the verdict honest:
 * once the encoder has finished cleanly, an upstream stage that then dies of a
 * broken pipe lost its reader, not its work — `-shortest` ends the encode a
 * fraction of a frame before the video stream does. A stage that dies *before*
 * the encoder finished is a failure even though the encoder then exits 0 on a
 * short stream, which is the case the rule exists for.
 */
import { spawn as nodeSpawn, type ChildProcess, type SpawnOptions } from 'node:child_process'
import { rmSync, writeFileSync } from 'node:fs'
import type { Readable } from 'node:stream'

import type { RenderStage } from '@shared/render'

import { LineTail, ProgressParser } from './progress'

/** A child of the pipeline, upstream first. */
export type PipelineStage = RenderStage

export interface StageCommand {
  stage: PipelineStage
  command: string
  args: string[]
}

export interface JobSpec {
  /** Upstream first; the last is the encoder, whose descriptor 3 is `-progress`. */
  stages: StageCommand[]
  /** The file the encoder writes, removed when the job does not finish. */
  output: string
  /** Where a failure's stderr tails are written. */
  log: string
}

export type JobOutcome =
  | { kind: 'done' }
  | { kind: 'cancelled' }
  | { kind: 'failed'; stage: PipelineStage; reason: string; log: string | undefined; tail: string[] }

export interface JobListener {
  /** The encoder's frame count moved. */
  onProgress?: (frame: number) => void
  /** One whole line of a stage's standard error. */
  onStderrLine?: (stage: PipelineStage, line: string) => void
}

/** `child_process.spawn`'s shape, so a test can hand in stub children. */
export type Spawner = (command: string, args: readonly string[], options: SpawnOptions) => ChildProcess

/** Lines of each stage's stderr kept for the log and the view. */
const TAIL_LINES = 20

interface Running {
  spec: StageCommand
  child: ChildProcess | undefined
  tail: LineTail
  settled: boolean
  /** Stopped by this job (a cancel, or another stage's failure), not failed. */
  stopped: boolean
}

export class RenderJob {
  private readonly running: Running[]
  private readonly progress = new ProgressParser()
  private failure: { stage: PipelineStage; reason: string } | undefined
  private cancelled = false
  private encoderClean = false
  private finish: ((outcome: JobOutcome) => void) | undefined

  constructor(
    private readonly spec: JobSpec,
    private readonly listener: JobListener = {},
    private readonly spawn: Spawner = nodeSpawn,
  ) {
    if (spec.stages.length < 2 || spec.stages[spec.stages.length - 1].stage !== 'encoder') {
      throw new Error('a render job ends in the encoder and has a stage before it')
    }
    this.running = spec.stages.map((stage) => ({
      spec: stage,
      child: undefined,
      tail: new LineTail(TAIL_LINES, (line) => listener.onStderrLine?.(stage.stage, line)),
      settled: false,
      stopped: false,
    }))
  }

  /** Frames the encoder has written. */
  get frame(): number {
    return this.progress.frame
  }

  /** Spawn every stage, and resolve once all of them have exited. */
  start(): Promise<JobOutcome> {
    const outcome = new Promise<JobOutcome>((resolve) => {
      this.finish = resolve
    })
    let upstream: Readable | null = null
    for (const [index, stage] of this.running.entries()) {
      const last = index === this.running.length - 1
      const stdin = upstream ?? 'ignore'
      const options: SpawnOptions = {
        stdio: last ? [stdin, 'ignore', 'pipe', 'pipe'] : [stdin, 'pipe', 'pipe'],
        windowsHide: true,
      }
      let child: ChildProcess
      try {
        child = this.spawn(stage.spec.command, stage.spec.args, options)
      } catch (error) {
        this.settle(index, null, null, error as Error)
        break
      }
      stage.child = child
      // The parent's copy of the pipe the child just took. Closing it is what
      // lets a dead reader reach its writer as a broken pipe.
      upstream?.destroy()
      upstream = last ? null : child.stdout
      child.stderr?.setEncoding('utf8')
      child.stderr?.on('data', (chunk: string) => stage.tail.push(chunk))
      if (last) {
        const channel = child.stdio[3] as Readable | null | undefined
        channel?.setEncoding('utf8')
        channel?.on('data', (chunk: string) => {
          if (this.progress.push(chunk)) this.listener.onProgress?.(this.progress.frame)
        })
      }
      child.once('error', (error) => this.settle(index, null, null, error))
      child.once('close', (code: number | null, signal: NodeJS.Signals | null) =>
        this.settle(index, code, signal),
      )
    }
    // A spawn that threw stops the stages that did start; nothing is waited on
    // for a stage that never existed.
    for (const stage of this.running) if (stage.child === undefined) stage.settled = true
    this.maybeFinish()
    return outcome
  }

  /** Stop every stage; the outcome resolves as `cancelled` once they exit. */
  cancel(): void {
    if (this.cancelled) return
    this.cancelled = true
    this.stopAll()
  }

  /**
   * Stop every stage and remove the partial output now, synchronously, for a
   * studio that is quitting and will not be there to wait for the exits.
   */
  abandon(): void {
    this.cancel()
    removeQuietly(this.spec.output)
  }

  private stopAll(): void {
    for (const stage of this.running) {
      if (stage.settled || stage.child === undefined) continue
      stage.stopped = true
      stage.child.kill('SIGTERM')
    }
  }

  private settle(
    index: number,
    code: number | null,
    signal: NodeJS.Signals | null,
    error?: Error,
  ): void {
    const stage = this.running[index]
    if (stage.settled) return
    stage.settled = true
    const last = index === this.running.length - 1
    const clean = error === undefined && code === 0
    if (last && clean) this.encoderClean = true

    const lostItsReader = !last && this.encoderClean
    if (!clean && !stage.stopped && !this.cancelled && !lostItsReader && this.failure === undefined) {
      this.failure = { stage: stage.spec.stage, reason: describeExit(code, signal, error, stage.tail) }
      this.stopAll()
    }
    if (last && clean && !this.progress.done && this.failure === undefined && !this.cancelled) {
      this.failure = {
        stage: 'encoder',
        reason: 'the encoder exited without reporting the end of its stream',
      }
    }
    this.maybeFinish()
  }

  private maybeFinish(): void {
    if (this.finish === undefined || this.running.some((stage) => !stage.settled)) return
    const finish = this.finish
    this.finish = undefined
    if (this.cancelled) {
      removeQuietly(this.spec.output)
      finish({ kind: 'cancelled' })
      return
    }
    if (this.failure !== undefined) {
      // An MP4 cut off before its index is written does not play; leaving one
      // behind would read as a short render.
      removeQuietly(this.spec.output)
      const failed = this.running.find((stage) => stage.spec.stage === this.failure?.stage)
      finish({
        kind: 'failed',
        stage: this.failure.stage,
        reason: this.failure.reason,
        log: this.writeLog(),
        tail: failed?.tail.lines() ?? [],
      })
      return
    }
    finish({ kind: 'done' })
  }

  /** Every stage's command line and stderr tail, beside the output. */
  private writeLog(): string | undefined {
    const blocks = this.running.map((stage) =>
      [
        `== ${stage.spec.stage}: ${[stage.spec.command, ...stage.spec.args].join(' ')}`,
        ...stage.tail.lines(),
      ].join('\n'),
    )
    const text = [
      `render failed in the ${this.failure?.stage ?? '?'} stage: ${this.failure?.reason ?? ''}`,
      ...blocks,
      '',
    ].join('\n\n')
    try {
      writeFileSync(this.spec.log, text, 'utf8')
      return this.spec.log
    } catch {
      return undefined
    }
  }
}

function describeExit(
  code: number | null,
  signal: NodeJS.Signals | null,
  error: Error | undefined,
  tail: LineTail,
): string {
  const what =
    error !== undefined
      ? `could not run: ${error.message}`
      : signal !== null
        ? `was killed by ${signal}`
        : `exited with code ${String(code)}`
  const said = tail.lines().at(-1)
  return said === undefined ? what : `${what} - ${said}`
}

function removeQuietly(path: string): void {
  try {
    rmSync(path, { force: true })
  } catch {
    // A file the encoder still holds open on Windows; the log names it.
  }
}
