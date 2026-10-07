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
import { existsSync, mkdirSync, rmSync } from 'node:fs'
import { readFile } from 'node:fs/promises'
import { basename, dirname, extname, join, resolve } from 'node:path'

import {
  renderRequestSchema,
  type DiffusionSettings,
  type OpenedJob,
  type Peaks,
  type PreparedTrack,
  type ProbeResult,
  type RenderEvent,
  type RenderRequest,
  type RenderResult,
  type SidecarPace,
} from '@shared/render'
import { timelineProblem } from '@shared/timeline'

import {
  encoderArgs,
  neuralFiles,
  playerRenderArgs,
  readBars,
  sidecarArgs,
  writeTimeline,
} from './commands'
import { readJob, writeJob } from './job'
import { peaksOfFile } from './peaks'
import { RenderJob, type Spawner, type StageCommand } from './pipeline'
import { probeDiffusion } from './probe'
import { cacheKey, runTool, type RunTool, type TranscodeCache } from './transcode'

export interface RenderEnvironment {
  /** The resolved player binary, or `undefined` when none was found. */
  player: () => string | undefined
  ffmpeg: () => string
  /** `render.diffusion` as the settings file holds it now. */
  diffusion: () => DiffusionSettings | undefined
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
  private readonly peaks = new Map<string, Promise<Peaks>>()
  private job: RenderJob | undefined
  private starting = false
  /**
   * Aborts the start in its preparation window — the transcode and the
   * `--bars` read — from the moment `start` passes its checks until it
   * launches the job or refuses.
   */
  private preparing: AbortController | undefined
  private probed: Promise<ProbeResult> | undefined

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
      return { ok: true, value: { source, grid, peaks: await this.peaksFor(wav) } }
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
    // Before anything is transcoded or spawned: a neural job with no prompt is
    // refused here, not an hour in by the sidecar.
    const neural = request.neural
    if (neural !== null) {
      const problem = timelineProblem(neural.timeline)
      if (problem !== undefined) return refuse(`the neural render cannot start: ${problem}`)
    }

    this.starting = true
    const abort = new AbortController()
    this.preparing = abort
    const run: RunTool = (command, args) => this.run(command, args, abort.signal)
    // The grid and the timeline written beside the output for a job that never
    // launched are removed: nothing would read them, and they would sit beside
    // a clip that was never rendered.
    let beside: string[] = []
    let launched = false
    try {
      let sidecar: StageCommand | undefined
      let diffusion: { python: string; script: string } | undefined
      if (neural !== null) {
        const probe = await this.probe(false)
        if (!probe.ready) return refuse(`the neural render cannot start: ${probe.reason}`)
        const { python, script } = this.env.diffusion() ?? {}
        if (python === undefined || script === undefined) {
          return refuse('the neural render cannot start: render.diffusion is not set')
        }
        diffusion = { python, script }
      }
      mkdirSync(dirname(request.output), { recursive: true })
      const wav = await this.env.cache.wavFor(request.source, abort.signal)
      let grid
      if (neural === null || diffusion === undefined) {
        grid = await readBars(run, player, wav, request.fps, this.barsFile(wav, request.fps))
      } else {
        // The grid is written beside the output by the player itself, so the
        // sidecar reads the bars the strip showed, byte for byte.
        const files = neuralFiles(request.output)
        beside = [files.bars, files.timeline]
        grid = await readBars(run, player, wav, request.fps, files.bars)
        const problem = timelineProblem(neural.timeline, grid.bar_starts.length)
        if (problem !== undefined) return refuse(`the neural render cannot start: ${problem}`)
        // A read that finished despite the abort writes no timeline.
        if (abort.signal.aborted) return refuse('the studio is quitting')
        writeTimeline(files.timeline, neural.timeline)
        sidecar = {
          stage: 'sidecar',
          command: diffusion.python,
          args: sidecarArgs(diffusion.script, neural, files),
        }
      }
      // A tool that finished despite the abort still launches nothing.
      if (abort.signal.aborted) return refuse('the studio is quitting')
      writeJob(request)
      this.launch(player, wav, request, grid.frames, sidecar)
      launched = true
      return { ok: true, value: null }
    } catch (error) {
      return refuse(abort.signal.aborted ? 'the studio is quitting' : (error as Error).message)
    } finally {
      if (!launched) for (const file of beside) rmSync(file, { force: true })
      this.preparing = undefined
      this.starting = false
    }
  }

  /**
   * The render a job file describes, with its output granted, and its track
   * granted when it is still there. A track that moved is reported rather than
   * refused, so the view asks for it again and keeps everything else.
   */
  async openJob(path: string): Promise<RenderResult<OpenedJob>> {
    try {
      const request = readJob(await readFile(path, 'utf8'))
      request.output = this.grant(request.output)
      const sourceMissing = !existsSync(request.source)
      if (!sourceMissing) request.source = this.grant(request.source)
      return { ok: true, value: { request, sourceMissing } }
    } catch (error) {
      return refuse((error as Error).message)
    }
  }

  /**
   * Whether the sidecar can run, asked once per session; `recheck` asks again
   * after the user fixed what the last answer named.
   */
  probe(recheck: boolean): Promise<ProbeResult> {
    if (recheck || this.probed === undefined) {
      this.probed = probeDiffusion(this.env.diffusion(), this.run)
    }
    return this.probed
  }

  /** Drop the cached answer, because a diffusion path changed under it. */
  forgetProbe(): void {
    this.probed = undefined
  }

  cancel(): void {
    this.job?.cancel()
  }

  /**
   * Stop whatever the studio is doing before it exits: a start still
   * transcoding or counting bars, and a running job, whose partial file is
   * removed.
   */
  abandon(): void {
    this.preparing?.abort()
    this.job?.abandon()
  }

  private launch(
    player: string,
    wav: string,
    request: RenderRequest,
    frames: number,
    sidecar: StageCommand | undefined,
  ): void {
    const stages: StageCommand[] = [
      { stage: 'player', command: player, args: playerRenderArgs(wav, request) },
      ...(sidecar === undefined ? [] : [sidecar]),
      { stage: 'encoder', command: this.env.ffmpeg(), args: encoderArgs(wav, request.output) },
    ]
    const now = this.env.now ?? Date.now
    const began = now()
    const pace = new SidecarMeter(now)
    let encoded = 0
    const report = (): void =>
      this.env.emit({
        kind: 'progress',
        frame: encoded,
        frames,
        elapsedMs: now() - began,
        ...(sidecar === undefined ? {} : { sidecar: pace.reading() }),
      })
    const job = new RenderJob(
      { stages, output: request.output, log: `${request.output}.render.log` },
      {
        onProgress: (frame) => {
          encoded = frame
          report()
        },
        onStderrLine: (stage, line) => {
          if (stage === 'sidecar' && pace.read(line)) report()
        },
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

  /**
   * The waveform of one transcode, read once: a new rate re-counts the bars but
   * draws the same file.
   */
  private peaksFor(wav: string): Promise<Peaks> {
    let peaks = this.peaks.get(wav)
    if (peaks === undefined) {
      peaks = peaksOfFile(wav)
      peaks.catch(() => this.peaks.delete(wav))
      this.peaks.set(wav, peaks)
    }
    return peaks
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

/**
 * The sidecar's own count, from the `sd-filter: N frames` line it prints every
 * ten frames, and its pace since the first such line. The first line is the
 * baseline rather than the process start, so the model load is not averaged
 * into the per-frame figure.
 */
export class SidecarMeter {
  private first: { at: number; frames: number } | undefined
  private latest: { at: number; frames: number } | undefined

  constructor(private readonly now: () => number) {}

  /** Read one stderr line; returns whether it was a count. */
  read(line: string): boolean {
    const match = /^sd-filter: (\d+) frames$/.exec(line.trim())
    if (match === null) return false
    const reading = { at: this.now(), frames: Number(match[1]) }
    this.first ??= reading
    this.latest = reading
    return true
  }

  reading(): SidecarPace {
    const frames = this.latest?.frames ?? 0
    if (this.first === undefined || this.latest === undefined) return { frames, secondsPerFrame: undefined }
    const done = this.latest.frames - this.first.frames
    return {
      frames,
      secondsPerFrame: done > 0 ? (this.latest.at - this.first.at) / 1000 / done : undefined,
    }
  }
}

function refuse<T>(reason: string): RenderResult<T> {
  return { ok: false, reason }
}
