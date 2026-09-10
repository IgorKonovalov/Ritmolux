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

export interface StudioSettings {
  /** An explicit player binary, second in ADR-0178's resolution order. */
  playerPath?: string
  /** Absent, or unreadable, means [`DEFAULT_PLAYER_MODE`]. */
  playerMode?: PlayerMode
}

export function settingsFile(userData: string): string {
  return join(userData, 'settings.json')
}

/** The mode to spawn with — the one place the default is applied. */
export function playerModeOf(settings: StudioSettings): PlayerMode {
  return settings.playerMode ?? DEFAULT_PLAYER_MODE
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
