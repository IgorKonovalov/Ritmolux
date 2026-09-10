/** The studio's own facts, and the one way a link leaves the window. */
import { ipcRenderer } from 'electron'

import { IPC_CHANNELS } from '@shared/ipc-channels'
import type { SchemaResult, SettingsResult } from '../../ipc/appHandlers'
import type { PlayerMode } from '@shared/player-mode'

export interface AppInfo {
  studioVersion: string
  playerPath: string | undefined
  playerSource: string | undefined
  playerMode: PlayerMode
}

export const appApi = {
  getInfo: (): Promise<AppInfo> =>
    ipcRenderer.invoke(IPC_CHANNELS.APP_GET_INFO) as Promise<AppInfo>,
  openExternal: (url: string): Promise<boolean> =>
    ipcRenderer.invoke(IPC_CHANNELS.SHELL_OPEN_EXTERNAL, url) as Promise<boolean>,

  /** The engine's parameter schema, or why it could not be read. */
  getSchema: (): Promise<SchemaResult> =>
    ipcRenderer.invoke(IPC_CHANNELS.APP_GET_SCHEMA) as Promise<SchemaResult>,

  /**
   * Remember which mode to spawn the player in. The mode is read at spawn, so
   * this takes effect on the next launch and never restarts a running show.
   */
  setPlayerMode: (mode: PlayerMode): Promise<SettingsResult> =>
    ipcRenderer.invoke(IPC_CHANNELS.APP_SET_PLAYER_MODE, mode) as Promise<SettingsResult>,
}
