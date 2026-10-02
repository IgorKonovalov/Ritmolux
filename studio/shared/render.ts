/**
 * The shapes a clip render crosses the process boundary in (ADR-0262).
 *
 * Main composes the pipeline and the renderer only describes it, so everything
 * here is either what the renderer asks for or what main reports back. Nothing
 * in this file touches Node: the renderer's project type-checks it too.
 */
import { z } from 'zod'

/** The two tiers `ritmolux --render` takes. */
export const RENDER_TIERS = ['floor', 'rich'] as const
export type RenderTier = (typeof RENDER_TIERS)[number]

/** The view's defaults: a 1080p clip at 30 fps on the rich tier. */
export const DEFAULT_RENDER_FPS = '30'
export const DEFAULT_RENDER_SIZE = '1920x1080'
export const DEFAULT_RENDER_TIER: RenderTier = 'rich'

/**
 * `settings.json`'s `render` key. Every field is optional: an absent
 * `ffmpegPath` is `ffmpeg` on `PATH`, an absent `outputDir` is the user's
 * Videos directory.
 */
export interface RenderSettings {
  ffmpegPath?: string
  outputDir?: string
}

/**
 * A rate as the player's `--fps` reads it: a whole number, or an exact
 * `num/den`. A decimal is not one — the player refuses it rather than round it.
 */
export const fpsSchema = z
  .string()
  .regex(/^[1-9]\d*(\/[1-9]\d*)?$/, 'a rate is a whole number or num/den, such as 30 or 30000/1001')

/** A frame size as `--size` reads it. */
export const sizeSchema = z.string().regex(/^[1-9]\d*x[1-9]\d*$/, 'a size is WxH, such as 1920x1080')

/** What Start sends: one clip, fully described. Validated in main. */
export const renderRequestSchema = z.object({
  source: z.string().min(1),
  preset: z.string().min(1),
  fps: fpsSchema,
  size: sizeSchema,
  tier: z.enum(RENDER_TIERS),
  output: z.string().min(1),
})
export type RenderRequest = z.infer<typeof renderRequestSchema>

/**
 * The player's `--bars` document: the first frame of every bar, bar 1 at frame
 * 0, and per bar whether the downbeat estimator placed it.
 */
export interface BarGrid {
  fps: string
  frames: number
  bar_starts: number[]
  bar_locked: boolean[]
}

const barGridSchema = z
  .object({
    fps: z.string(),
    frames: z.number().int().positive(),
    bar_starts: z.array(z.number().int().nonnegative()).min(1),
    bar_locked: z.array(z.boolean()),
  })
  .strict()

/**
 * Validate a `--bars` document, refusing what `sd_filter.py`'s `parse_bar_grid`
 * refuses, so a grid the strip draws is a grid the sidecar will read.
 */
export function parseBarGrid(json: unknown): BarGrid {
  const parsed = barGridSchema.safeParse(json)
  if (!parsed.success) {
    const first = parsed.error.issues[0]
    throw new Error(`the bar grid is not one the studio reads: ${first.path.join('.')} ${first.message}`)
  }
  const grid = parsed.data
  if (grid.bar_starts[0] !== 0) throw new Error('the bar grid does not start at frame 0')
  for (let n = 1; n < grid.bar_starts.length; n += 1) {
    if (grid.bar_starts[n] <= grid.bar_starts[n - 1]) {
      throw new Error(`the bar grid's bar ${n + 1} does not start after bar ${n}`)
    }
  }
  if (grid.bar_starts[grid.bar_starts.length - 1] >= grid.frames) {
    throw new Error('the bar grid has a bar starting past its last frame')
  }
  if (grid.bar_locked.length !== grid.bar_starts.length) {
    throw new Error('the bar grid does not carry one bar_locked per bar')
  }
  return grid
}

/** A waveform's lowest and highest sample per column, each in [-1, 1]. */
export interface Peaks {
  min: number[]
  max: number[]
}

/** A chosen track as main prepared it: transcoded, its bars counted, its waveform read. */
export interface PreparedTrack {
  source: string
  grid: BarGrid
  peaks: Peaks
}

/** Which child of the pipeline a failure belongs to, upstream first. */
export type RenderStage = 'player' | 'sidecar' | 'encoder'

/** What main pushes while a job runs, and once when it ends. */
export type RenderEvent =
  | { kind: 'started'; output: string; frames: number }
  | { kind: 'progress'; frame: number; frames: number; elapsedMs: number }
  | { kind: 'done'; output: string }
  | { kind: 'cancelled' }
  | { kind: 'failed'; stage: RenderStage; reason: string; log: string | undefined; tail: string[] }

/** The answer every render call resolves to: never a rejection. */
export type RenderResult<T> = { ok: true; value: T } | { ok: false; reason: string }

/**
 * Seconds left, from frames done in the time taken so far, or `undefined`
 * before the first frame lands.
 */
export function etaSeconds(frame: number, frames: number, elapsedMs: number): number | undefined {
  if (frame <= 0 || frames <= frame) return frame >= frames && frames > 0 ? 0 : undefined
  return Math.round(((elapsedMs / frame) * (frames - frame)) / 1000)
}

/** `1:02:05` or `4:09`, for a count of seconds. */
export function formatDuration(seconds: number): string {
  const s = Math.max(0, Math.round(seconds))
  const h = Math.floor(s / 3600)
  const m = Math.floor((s % 3600) / 60)
  const rest = String(s % 60).padStart(2, '0')
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${rest}` : `${m}:${rest}`
}
