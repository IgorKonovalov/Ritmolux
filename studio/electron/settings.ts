/**
 * The studio's own settings file, in the per-user application directory.
 *
 * Read at start and written only when the user changes one of them. A file that
 * is missing, is not JSON, or carries a key of the wrong shape degrades to "no
 * setting" rather than failing the launch: `playerPath`'s resolution order has
 * two other roots and `playerMode` has a default, and a studio that refuses to
 * open because a hand-edited file has a trailing comma would be the worse
 * failure. Parsed at the boundary, never asserted into shape.
 */
import { mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'

import { DEFAULT_PLAYER_MODE, isPlayerMode, type PlayerMode } from '@shared/player-mode'
import type { DiffusionSettings, RenderSettingKey, RenderSettings } from '@shared/render'

export interface StudioSettings {
  /** An explicit player binary, second in ADR-0178's resolution order. */
  playerPath?: string
  /** Absent, or unreadable, means [`DEFAULT_PLAYER_MODE`]. */
  playerMode?: PlayerMode
  /** How the window looks and moves; nothing in it reaches the player. */
  ui?: UiSettings
  /** Where a clip render finds its encoder and puts its file (ADR-0262). */
  render?: RenderSettings
  /** Where a judging session draws its sets from (ADR-0267). */
  judging?: JudgingSettings
}

export interface JudgingSettings {
  /** A preset directory, typically a checkout's `presets/`. Absent means none chosen. */
  sourceDir?: string
}

export interface UiSettings {
  /** Absent means `false`. Either this or the system's preference stops motion. */
  reducedMotion?: boolean
}

export function settingsFile(userData: string): string {
  return join(userData, 'settings.json')
}

/** The mode to spawn with — the one place the default is applied. */
export function playerModeOf(settings: StudioSettings): PlayerMode {
  return settings.playerMode ?? DEFAULT_PLAYER_MODE
}

/** Whether the studio's own motion is off — the one place its default is applied. */
export function reducedMotionOf(settings: StudioSettings): boolean {
  return settings.ui?.reducedMotion ?? false
}

/** A path-valued key: a non-empty string, or absent. */
function pathValue(value: unknown): string | undefined {
  return typeof value === 'string' && value.trim() !== '' ? value : undefined
}

/**
 * The `render` object, keeping each key that has the right shape and dropping
 * the rest, so a mistyped `outputDir` costs that key and not `ffmpegPath`.
 */
function readRender(value: unknown): RenderSettings | undefined {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined
  const record = value as Record<string, unknown>
  const render: RenderSettings = {}
  const ffmpegPath = pathValue(record.ffmpegPath)
  if (ffmpegPath !== undefined) render.ffmpegPath = ffmpegPath
  const outputDir = pathValue(record.outputDir)
  if (outputDir !== undefined) render.outputDir = outputDir
  const diffusion = readDiffusion(record.diffusion)
  if (diffusion !== undefined) render.diffusion = diffusion
  return Object.keys(render).length === 0 ? undefined : render
}

/** `render.diffusion`, kept key by key the same way. */
function readDiffusion(value: unknown): DiffusionSettings | undefined {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined
  const record = value as Record<string, unknown>
  const diffusion: DiffusionSettings = {}
  const python = pathValue(record.python)
  if (python !== undefined) diffusion.python = python
  const script = pathValue(record.script)
  if (script !== undefined) diffusion.script = script
  return Object.keys(diffusion).length === 0 ? undefined : diffusion
}

/** The `judging` object, kept key by key like `render`. */
function readJudging(value: unknown): JudgingSettings | undefined {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) return undefined
  const sourceDir = pathValue((value as Record<string, unknown>).sourceDir)
  return sourceDir === undefined ? undefined : { sourceDir }
}

/** The directory judging sets are drawn from, or `undefined` when none is set. */
export function sourceDirOf(settings: StudioSettings): string | undefined {
  return settings.judging?.sourceDir
}

/** `settings` with `judging.sourceDir` set, or removed by `null` or an empty path. */
export function withJudgingSource(settings: StudioSettings, sourceDir: string | null): StudioSettings {
  const judging = readJudging({ ...settings.judging, sourceDir })
  const rest: StudioSettings = { ...settings }
  delete rest.judging
  return judging === undefined ? rest : { ...rest, judging }
}

/** The `ffmpeg` a render runs — the one place its default is applied. */
export function ffmpegOf(settings: StudioSettings): string {
  return settings.render?.ffmpegPath ?? 'ffmpeg'
}

/** Where a render's file goes by default, given the user's Videos directory. */
export function outputDirOf(settings: StudioSettings, videos: string): string {
  return settings.render?.outputDir ?? videos
}

/**
 * `next` merged onto the `render` key of `settings`, with an empty or `null`
 * value removing its key: clearing a field in the panel means "the default".
 * A `diffusion.*` key names the nested object's field.
 */
export function withRender(
  settings: StudioSettings,
  next: Partial<Record<RenderSettingKey, string | null>>,
): StudioSettings {
  const merged: Record<string, unknown> = { ...settings.render }
  const diffusion: Record<string, unknown> = { ...settings.render?.diffusion }
  for (const [key, value] of Object.entries(next)) {
    const [target, field] = key.startsWith('diffusion.')
      ? [diffusion, key.slice('diffusion.'.length)]
      : [merged, key]
    if (value === null || value === undefined || value.trim() === '') delete target[field]
    else target[field] = value
  }
  merged.diffusion = diffusion
  const render = readRender(merged)
  const rest: StudioSettings = { ...settings }
  delete rest.render
  return render === undefined ? rest : { ...rest, render }
}

export function readSettings(file: string): StudioSettings {
  let raw: string
  try {
    raw = readFileSync(file, 'utf8')
  } catch {
    return {}
  }
  try {
    const parsed: unknown = JSON.parse(raw)
    if (typeof parsed !== 'object' || parsed === null) return {}
    const record = parsed as Record<string, unknown>
    const settings: StudioSettings = {}
    if (typeof record.playerPath === 'string') settings.playerPath = record.playerPath
    // A mode this build does not know reads as absent, not as an error: the
    // rest of the file is still usable, and the default is a working answer.
    if (isPlayerMode(record.playerMode)) settings.playerMode = record.playerMode
    const ui = record.ui
    if (typeof ui === 'object' && ui !== null && !Array.isArray(ui)) {
      const reducedMotion = (ui as Record<string, unknown>).reducedMotion
      if (typeof reducedMotion === 'boolean') settings.ui = { reducedMotion }
    }
    const render = readRender(record.render)
    if (render !== undefined) settings.render = render
    const judging = readJudging(record.judging)
    if (judging !== undefined) settings.judging = judging
    return settings
  } catch {
    return {}
  }
}

/**
 * Write the settings back, atomically.
 *
 * A temporary file and a rename, for the same reason a preset save uses one:
 * this file is read at every launch, and a partial write is a studio that has
 * lost its player path. The keys it does not understand are **not** preserved —
 * it writes the settings it was handed — so a caller merges onto what
 * `readSettings` returned rather than onto nothing.
 */
export function writeSettings(file: string, settings: StudioSettings): void {
  mkdirSync(dirname(file), { recursive: true })
  const temporary = `${file}.tmp`
  writeFileSync(temporary, `${JSON.stringify(settings, null, 2)}\n`, 'utf8')
  renameSync(temporary, file)
}
