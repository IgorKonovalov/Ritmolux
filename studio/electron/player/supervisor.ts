/**
 * The player child: spawn it, read both its pipes, kill it on quit.
 *
 * One process at a time. Standard error carries the events, standard output
 * carries the frames, and the geometry that separates one frame from the next
 * arrives as an event — so the splitter cannot be built until `stream` has been
 * seen, and output that arrives before it is held rather than guessed at. The
 * two pipes are independent, so nothing guarantees the event is read first even
 * though the player writes it first.
 *
 * The supervisor holds no editing logic and knows nothing about presets: it
 * moves bytes and hands typed values up (ADR-0178).
 */
import { spawn as nodeSpawn } from 'node:child_process'
import type { Readable } from 'node:stream'

import { frameBytes, isKnownPlayerVersion, type PlayerEvent } from '@shared/protocol'

import { EventReader } from './events'
import { FramePump, FrameSplitter } from './frames'

/**
 * The invocation the studio asks for.
 *
 * `--control` is requested rather than assumed: the player answers with the
 * address it actually bound in `hello.control`, or `null` when it opened no
 * listener, and the studio reads that answer instead of predicting it. Port `0`
 * asks for an ephemeral one, which is why the answer is the only source.
 */
export const DEFAULT_PLAYER_ARGS = [
  '--stream',
  '--sink',
  'stdout',
  '--events',
  '--control',
  '127.0.0.1:0',
] as const

/**
 * The child, narrowed to the four things the supervisor uses.
 *
 * Narrow rather than `ChildProcess` so a test can stand in a pair of streams:
 * the supervisor's whole job is reading two pipes, and a fake that has to
 * satisfy the full class would be testing Node rather than this file.
 */
export interface PlayerChild {
  stdout: Readable
  stderr: Readable
  kill(): boolean
  on(event: 'exit', listener: (code: number | null, signal: NodeJS.Signals | null) => void): unknown
}

export type SpawnFn = (command: string, args: readonly string[]) => PlayerChild

export interface SupervisorSinks {
  onEvent: (event: PlayerEvent) => void
  onDiagnostic: (line: string) => void
  onMalformed: (line: string, reason: string) => void
  /** The player is not one this studio drives; it has been stopped. */
  onRefused: (reason: string) => void
  onExit: (code: number | null, signal: NodeJS.Signals | null) => void
}

export interface SupervisorOptions extends SupervisorSinks {
  playerPath: string
  args?: readonly string[]
  spawn?: SpawnFn
}

export class PlayerSupervisor {
  private child: PlayerChild | undefined
  private splitter: FrameSplitter | undefined
  /** Output read before `stream` said how big a frame is. */
  private preStream: Buffer[] = []
  private stopping = false

  readonly pump = new FramePump()

  private readonly reader: EventReader

  constructor(private readonly options: SupervisorOptions) {
    this.reader = new EventReader({
      onEvent: (event) => this.event(event),
      onDiagnostic: options.onDiagnostic,
      onMalformed: options.onMalformed,
    })
  }

  /** The control address the player reported, once `hello` has arrived. */
  private controlAddress: string | null = null

  get control(): string | null {
    return this.controlAddress
  }

  start(): void {
    if (this.child !== undefined) throw new Error('the player is already running')
    const spawnFn = this.options.spawn ?? defaultSpawn
    const child = spawnFn(this.options.playerPath, this.options.args ?? DEFAULT_PLAYER_ARGS)
    this.child = child

    child.stdout.on('data', (chunk: Buffer) => this.output(chunk))
    child.stderr.setEncoding('utf8')
    child.stderr.on('data', (chunk: string) => this.reader.push(chunk))
    child.on('exit', (code, signal) => {
      this.reader.flush()
      this.child = undefined
      if (!this.stopping) this.options.onExit(code, signal)
    })
  }

  stop(): void {
    this.stopping = true
    this.pump.detach()
    this.child?.kill()
    this.child = undefined
  }

  private event(event: PlayerEvent): void {
    if (event.ev === 'hello') {
      this.controlAddress = event.control
      if (!isKnownPlayerVersion(event.version)) {
        // Refused rather than driven: a preview that silently shows a build the
        // panels were not generated from is worse than an empty one.
        this.options.onEvent(event)
        this.options.onRefused(
          `the player at ${this.options.playerPath} reports version ${event.version}, ` +
            `which this studio does not drive`,
        )
        this.stop()
        return
      }
    }
    if (event.ev === 'stream') {
      const bytes = frameBytes(event)
      this.splitter = new FrameSplitter(bytes, (frame) => this.pump.post(frame))
      const held = this.preStream
      this.preStream = []
      for (const chunk of held) this.splitter.push(chunk)
    }
    this.options.onEvent(event)
  }

  private output(chunk: Buffer): void {
    if (this.splitter === undefined) {
      this.preStream.push(chunk)
      return
    }
    this.splitter.push(chunk)
  }
}

function defaultSpawn(command: string, args: readonly string[]): PlayerChild {
  // Standard input is closed: the player reads none, and leaving it open would
  // hold a handle the studio has no use for.
  return nodeSpawn(command, [...args], { stdio: ['ignore', 'pipe', 'pipe'] })
}
