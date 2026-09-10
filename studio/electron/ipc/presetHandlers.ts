/**
 * The preset file's OS surface: read one, write one back.
 *
 * Not domain traffic — no message of the control protocol crosses here
 * (ADR-0178). What it needs main for is the filesystem, which the sandboxed
 * renderer has no access to and should not.
 *
 * **A path is only accepted when the player named it.** The renderer sends a
 * path back, and a renderer that has been compromised would otherwise have main
 * as a general-purpose file writer over the whole disk. The guard is the set of
 * paths the player itself reported — the preset on screen, and anything inside
 * the directory the `roster` event named — so the studio can write only where
 * the player is already looking.
 */
import { ipcMain } from 'electron'
import { resolve, sep } from 'node:path'

import { IPC_CHANNELS } from '@shared/ipc-channels'

import { readPreset, writePresetAtomically } from '../preset/writer'

export type PresetResult<T> = { ok: true; value: T } | { ok: false; reason: string }

/** The paths the player has named: the file on screen and the watched root. */
export interface PresetScope {
  file: string | undefined
  dir: string | undefined
}

/**
 * Whether `candidate` is a preset file the player told us about.
 *
 * `resolve` first, so `..` cannot walk out of the directory after the prefix
 * test; the separator is appended to the root so `/presets-other` is not
 * accepted as being inside `/presets`.
 */
export function isInScope(candidate: string, scope: PresetScope): boolean {
  if (!candidate.toLowerCase().endsWith('.toml')) return false
  const full = resolve(candidate)
  if (scope.file !== undefined && resolve(scope.file) === full) return true
  if (scope.dir === undefined) return false
  const root = resolve(scope.dir)
  return full.startsWith(root.endsWith(sep) ? root : root + sep)
}

export function registerPresetHandlers(scope: () => PresetScope): void {
  ipcMain.handle(
    IPC_CHANNELS.PRESET_READ,
    async (_event, path: unknown): Promise<PresetResult<{ path: string; text: string }>> => {
      if (typeof path !== 'string' || !isInScope(path, scope())) {
        return { ok: false, reason: 'that file is not one the player named' }
      }
      try {
        return { ok: true, value: await readPreset(path) }
      } catch (error) {
        return { ok: false, reason: (error as Error).message }
      }
    },
  )

  ipcMain.handle(
    IPC_CHANNELS.PRESET_WRITE,
    async (_event, path: unknown, text: unknown): Promise<PresetResult<null>> => {
      if (typeof path !== 'string' || !isInScope(path, scope())) {
        return { ok: false, reason: 'that file is not one the player named' }
      }
      if (typeof text !== 'string') return { ok: false, reason: 'a preset is written as text' }
      try {
        await writePresetAtomically(path, text)
        return { ok: true, value: null }
      } catch (error) {
        return { ok: false, reason: (error as Error).message }
      }
    },
  )
}
