/**
 * The OS surface: what the sandboxed renderer cannot do for itself.
 *
 * Deliberately small (ADR-0178). Nothing here is domain traffic — the player's
 * messages travel on the three domain channels and nowhere else.
 */
import { ipcMain, shell } from 'electron'

import { IPC_CHANNELS } from '@shared/ipc-channels'
import { isPlayerMode, type PlayerMode } from '@shared/player-mode'
import type { SchemaDocument } from '@shared/schema'

export interface AppInfo {
  studioVersion: string
  /** The binary that was resolved, and which root it came from. */
  playerPath: string | undefined
  playerSource: string | undefined
  /** The mode the running player was spawned in (ADR-0186). */
  playerMode: PlayerMode
}

/** What the renderer gets back when the schema could not be read. */
export type SchemaResult = { ok: true; document: SchemaDocument } | { ok: false; reason: string }

/** What the renderer gets back from a settings write. */
export type SettingsResult = { ok: true } | { ok: false; reason: string }

export function registerAppHandlers(
  info: () => AppInfo,
  schema: () => Promise<SchemaDocument>,
  setPlayerMode: (mode: PlayerMode) => void,
): void {
  ipcMain.handle(IPC_CHANNELS.APP_GET_INFO, () => info())
  ipcMain.handle(IPC_CHANNELS.APP_GET_SCHEMA, async (): Promise<SchemaResult> => {
    // Resolved, never rejected: a panel that cannot be built is a message on
    // screen, and an IPC rejection reaches the renderer as an opaque Error
    // whose text the user cannot act on.
    try {
      return { ok: true, document: await schema() }
    } catch (error) {
      return { ok: false, reason: (error as Error).message }
    }
  })
  ipcMain.handle(IPC_CHANNELS.APP_SET_PLAYER_MODE, (_event, mode: unknown): SettingsResult => {
    // Validated here rather than trusted: this is the boundary, and the value
    // is written into a file every future launch reads.
    if (!isPlayerMode(mode))
      return { ok: false, reason: `\`${String(mode)}\` is not a player mode` }
    try {
      setPlayerMode(mode)
      return { ok: true }
    } catch (error) {
      return { ok: false, reason: (error as Error).message }
    }
  })
  ipcMain.handle(IPC_CHANNELS.SHELL_OPEN_EXTERNAL, async (_event, url: unknown) => {
    // Only https leaves the app: a `file:` or a custom scheme handed to the OS
    // opener is an arbitrary-program launch with the renderer choosing the
    // program.
    if (typeof url !== 'string' || !url.startsWith('https://')) return false
    await shell.openExternal(url)
    return true
  })
}
