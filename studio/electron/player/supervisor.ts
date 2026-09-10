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

import {
  frameBytes,
  isKnownPixelFormat,
  isKnownPlayerVersion,
  type PlayerEvent,
} from '@shared/protocol'

import { EventReader } from './events'
import { FramePump, FrameSplitter } from './frames'

/**
 * The invocation the studio asks for.
 *
 * **One player, windowed, and the studio's picture is a copy of the show's**
 * (ADR-0181). `--preview stdout` mirrors the frames a window is already drawing
 * onto the same pipe format a headless run writes, so what the author edits is
 * by construction what the audience is watching: one capture, one adapter, one
 * preset directory, one rotation state. A second child rendering its own
 * preview is a preview that can differ from the show.
 *
 * `--control` is requested rather than assumed: the player answers with the
 * address it actually bound in `hello.control`, or `null` when it opened no
 * listener, and the studio reads that answer instead of predicting it. Port `0`
 * asks for an ephemeral one, which is why the answer is the only source.
 *
 * No size and no rate are named here, and none may be: a windowed run is paced
 * by its swapchain and reads back at its surface's size, so the geometry is the
 * player's to report and the `stream` event is where it reports it.
 */
export const DEFAULT_PLAYER_ARGS = [
  '--preview',
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
  /**
   * Something the studio will not drive, named. A player whose version it does
   * not know has been stopped; a frame pipe whose channel order it cannot name
   * is left unread while the player goes on drawing.
   */
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
  /** The pipe was refused, so its bytes are read and dropped rather than held. */
  private unreadable = false
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
      if (!isKnownPixelFormat(event.format)) {
        // No splitter, so nothing is posted and nothing is painted on a guess.
        // The player keeps drawing: it is still a valid show, and taking the
        // projector down because the studio cannot name a channel order would
        // be the worse failure. The renderer shows the refusal where the
        // picture would be.
        this.preStream = []
        this.unreadable = true
        this.options.onRefused(
          `the player announced its frames as \`${event.format}\`, ` +
            `a channel order this studio does not know; the preview is not painted`,
        )
        this.options.onEvent(event)
        return
      }
      const bytes = frameBytes(event)
      this.splitter = new FrameSplitter(bytes, (frame) => this.pump.post(frame))
      const held = this.preStream
      this.preStream = []
      for (const chunk of held) this.splitter.push(chunk)
    }
    this.options.onEvent(event)
  }

  private output(chunk: Buffer): void {
    // Dropped rather than held: without this the pre-stream list would grow for
    // as long as a refused player keeps writing, which is the whole run.
    if (this.unreadable) return
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
