/**
 * The clip render as main runs it: the paths it will touch, the session's
 * transcodes, and at most one job (ADR-0262).
 *
 * Electron stays out of this file — the dialogs, the IPC handlers and the
 * power-save blocker are `ipc/renderHandlers.ts`'s — so the rules here run
 * under a plain test.
 *
 * **A path is accepted only when main produced it.** The renderer hands paths
 * back, and without this guard main would transcode, overwrite and delete
 * wherever a compromised renderer pointed it. The granted set is what a dialog
 * answered and what this service suggested, the same rule the preset channels
 * hold with the player's own answers.
 */
import { mkdirSync } from 'node:fs'
import { basename, dirname, extname, join, resolve } from 'node:path'

import {
  renderRequestSchema,
  type PreparedTrack,
  type RenderEvent,
  type RenderRequest,
  type RenderResult,
} from '@shared/render'

import { encoderArgs, playerRenderArgs, readBars } from './commands'
import { RenderJob, type Spawner, type StageCommand } from './pipeline'
import { cacheKey, runTool, type RunTool, type TranscodeCache } from './transcode'

export interface RenderEnvironment {
  /** The resolved player binary, or `undefined` when none was found. */
  player: () => string | undefined
  ffmpeg: () => string
  /** `render.outputDir`, or the user's Videos directory. */
  outputDir: () => string
  cache: TranscodeCache
  emit: (event: RenderEvent) => void
  /** Hold the machine awake while a job runs; returns the release. */
  stayAwake: () => () => void
  run?: RunTool
  spawn?: Spawner
  now?: () => number
}

/** The file name a render of `source` with `preset` is suggested under. */
export function suggestedName(source: string, preset: string): string {
  const slug = (text: string): string =>
    text
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, '-')
      .replace(/^-+|-+$/g, '')
  const track = slug(basename(source, extname(source))) || 'track'
  return `${track}-${slug(preset) || 'preset'}.mp4`
}

export class RenderService {
  private readonly granted = new Set<string>()
  private job: RenderJob | undefined
  private starting = false

  constructor(private readonly env: RenderEnvironment) {}

  /** Whether a job is running, which is what a quit asks about. */
  get busy(): boolean {
    return this.job !== undefined || this.starting
  }

  /** Accept `path` from the renderer from now on. */
  grant(path: string): string {
    const full = resolve(path)
    this.granted.add(full)
    return full
  }

  isGranted(path: unknown): path is string {
    return typeof path === 'string' && this.granted.has(resolve(path))
  }

  /** Transcode `source` and count its bars at `fps`. */
  async prepare(source: unknown, fps: unknown): Promise<RenderResult<PreparedTrack>> {
    if (!this.isGranted(source)) return refuse('that track was not picked in this studio')
    const rate = renderRequestSchema.shape.fps.safeParse(fps)
    if (!rate.success) return refuse(rate.error.issues[0].message)
    const player = this.env.player()
    if (player === undefined) return refuse('no player was found, so the bars cannot be counted')
    try {
      const wav = await this.env.cache.wavFor(source)
      const grid = await readBars(this.run, player, wav, rate.data, this.barsFile(wav, rate.data))
      return { ok: true, value: { source, grid } }
    } catch (error) {
      return refuse((error as Error).message)
    }
  }

  /** `<outputDir>/<track>-<preset>.mp4`, granted. */
  suggestOutput(source: unknown, preset: unknown): RenderResult<string> {
    if (typeof source !== 'string' || typeof preset !== 'string') {
      return refuse('a suggestion needs a track and a preset')
    }
    return { ok: true, value: this.grant(join(this.env.outputDir(), suggestedName(source, preset))) }
  }

  /** Start one job, or say why not. Resolves once the children are spawned. */
  async start(input: unknown): Promise<RenderResult<null>> {
    if (this.busy) return refuse('a render is already running')
    const parsed = renderRequestSchema.safeParse(input)
    if (!parsed.success) {
      const first = parsed.error.issues[0]
      return refuse(`${first.path.join('.')}: ${first.message}`)
    }
    const request = parsed.data
    if (!this.isGranted(request.source)) return refuse('that track was not picked in this studio')
    if (!this.isGranted(request.output)) return refuse('that output was not chosen in this studio')
    const player = this.env.player()
    if (player === undefined) return refuse('no player was found to render with')

    this.starting = true
    try {
      mkdirSync(dirname(request.output), { recursive: true })
      const wav = await this.env.cache.wavFor(request.source)
      const grid = await readBars(this.run, player, wav, request.fps, this.barsFile(wav, request.fps))
      this.launch(player, wav, request, grid.frames)
      return { ok: true, value: null }
    } catch (error) {
      return refuse((error as Error).message)
    } finally {
      this.starting = false
    }
  }

  cancel(): void {
    this.job?.cancel()
  }

  /** Stop a running job and remove its partial file before the studio exits. */
  abandon(): void {
    this.job?.abandon()
  }

  private launch(player: string, wav: string, request: RenderRequest, frames: number): void {
    const stages: StageCommand[] = [
      { stage: 'player', command: player, args: playerRenderArgs(wav, request) },
      { stage: 'encoder', command: this.env.ffmpeg(), args: encoderArgs(wav, request.output) },
    ]
    const now = this.env.now ?? Date.now
    const began = now()
    const job = new RenderJob(
      { stages, output: request.output, log: `${request.output}.render.log` },
      {
        onProgress: (frame) =>
          this.env.emit({ kind: 'progress', frame, frames, elapsedMs: now() - began }),
      },
      this.env.spawn,
    )
    this.job = job
    const release = this.env.stayAwake()
    this.env.emit({ kind: 'started', output: request.output, frames })
    void job.start().then((outcome) => {
      release()
      this.job = undefined
      if (outcome.kind === 'done') this.env.emit({ kind: 'done', output: request.output })
      else if (outcome.kind === 'cancelled') this.env.emit({ kind: 'cancelled' })
      else this.env.emit({ ...outcome, kind: 'failed' })
    })
  }

  /** The `--bars` answer for one transcode at one rate, kept beside it. */
  private barsFile(wav: string, fps: string): string {
    const key = cacheKey(wav, 0, 0)
    return join(this.env.cache.directory, `${key}-${fps.replace('/', '_')}.bars.json`)
  }

  private get run(): RunTool {
    return this.env.run ?? runTool
  }
}

function refuse<T>(reason: string): RenderResult<T> {
  return { ok: false, reason }
}
