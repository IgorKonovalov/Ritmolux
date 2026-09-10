/**
 * The studio's own settings file, in the per-user application directory.
 *
 * Read at start, never written by this phase. A file that is missing, is not
 * JSON, or carries a `playerPath` that is not a string degrades to "no setting"
 * rather than failing the launch: the resolution order has two other roots, and
 * a studio that refuses to open because a hand-edited file has a trailing comma
 * would be the worse failure. Parsed at the boundary, never asserted into shape.
 */
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

export interface StudioSettings {
  /** An explicit player binary, second in ADR-0178's resolution order. */
  playerPath?: string
}

export function settingsFile(userData: string): string {
  return join(userData, 'settings.json')
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
    const playerPath = (parsed as Record<string, unknown>).playerPath
    return typeof playerPath === 'string' ? { playerPath } : {}
  } catch {
    return {}
  }
}
