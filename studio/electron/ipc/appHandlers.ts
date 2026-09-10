/**
 * The OS surface: what the sandboxed renderer cannot do for itself.
 *
 * Deliberately small (ADR-0178). Nothing here is domain traffic — the player's
 * messages travel on the three domain channels and nowhere else.
 */
import { ipcMain, shell } from 'electron'

import { IPC_CHANNELS } from '@shared/ipc-channels'
import type { SchemaDocument } from '@shared/schema'

export interface AppInfo {
  studioVersion: string
  /** The binary that was resolved, and which root it came from. */
  playerPath: string | undefined
  playerSource: string | undefined
}

/** What the renderer gets back when the schema could not be read. */
export type SchemaResult =
  | { ok: true; document: SchemaDocument }
  | { ok: false; reason: string }

export function registerAppHandlers(
  info: () => AppInfo,
  schema: () => Promise<SchemaDocument>,
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
  ipcMain.handle(IPC_CHANNELS.SHELL_OPEN_EXTERNAL, async (_event, url: unknown) => {
    // Only https leaves the app: a `file:` or a custom scheme handed to the OS
    // opener is an arbitrary-program launch with the renderer choosing the
    // program.
    if (typeof url !== 'string' || !url.startsWith('https://')) return false
    await shell.openExternal(url)
    return true
  })
}
