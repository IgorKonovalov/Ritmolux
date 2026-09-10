/** The studio's own facts, and the one way a link leaves the window. */
import { ipcRenderer } from 'electron'

import { IPC_CHANNELS } from '@shared/ipc-channels'

export interface AppInfo {
  studioVersion: string
  playerPath: string | undefined
  playerSource: string | undefined
}

export const appApi = {
  getInfo: (): Promise<AppInfo> =>
    ipcRenderer.invoke(IPC_CHANNELS.APP_GET_INFO) as Promise<AppInfo>,
  openExternal: (url: string): Promise<boolean> =>
    ipcRenderer.invoke(IPC_CHANNELS.SHELL_OPEN_EXTERNAL, url) as Promise<boolean>,
}
