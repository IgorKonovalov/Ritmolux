/**
 * The bridge: one `window.api`, and the frame port's hand-off.
 *
 * The renderer never sees `require`, `process`, `ipcRenderer` or a Node global
 * — only the typed surface exposed here (ADR-0178).
 */
import { contextBridge, ipcRenderer, type IpcRendererEvent } from 'electron'

import { FRAME_PORT_SENTINEL, IPC_CHANNELS } from '@shared/ipc-channels'

import { appApi } from './api/app'
import { presetApi } from './api/preset'
import { playerApi } from './api/player'

const api = {
  app: appApi,
  player: playerApi,
  preset: presetApi,
} as const

contextBridge.exposeInMainWorld('api', api)

export type ElectronAPI = typeof api

// The port arrives once, from main, and is moved straight into the main world.
// `window` here is the isolated world's, and `postMessage` with a transfer list
// is the one path a `MessagePort` can take across the boundary.
ipcRenderer.on(IPC_CHANNELS.PLAYER_FRAME, (event: IpcRendererEvent) => {
  window.postMessage(FRAME_PORT_SENTINEL, '*', event.ports)
})
