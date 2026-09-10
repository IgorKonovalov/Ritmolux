/**
 * The OS surface: what the sandboxed renderer cannot do for itself.
 *
 * Deliberately small (ADR-0178). Nothing here is domain traffic — the player's
 * messages travel on the three domain channels and nowhere else.
 */
import { ipcMain, shell } from 'electron'

import { IPC_CHANNELS } from '@shared/ipc-channels'

export interface AppInfo {
  studioVersion: string
  /** The binary that was resolved, and which root it came from. */
  playerPath: string | undefined
  playerSource: string | undefined
}

export function registerAppHandlers(info: () => AppInfo): void {
  ipcMain.handle(IPC_CHANNELS.APP_GET_INFO, () => info())
  ipcMain.handle(IPC_CHANNELS.SHELL_OPEN_EXTERNAL, async (_event, url: unknown) => {
    // Only https leaves the app: a `file:` or a custom scheme handed to the OS
    // opener is an arbitrary-program launch with the renderer choosing the
    // program.
    if (typeof url !== 'string' || !url.startsWith('https://')) return false
    await shell.openExternal(url)
    return true
  })
}
