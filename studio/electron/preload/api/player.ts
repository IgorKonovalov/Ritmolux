/**
 * The player's side of `window.api`.
 *
 * Events cross as plain objects and are handed to a callback; the binding
 * returns its own cleanup, so a component that subscribes can unsubscribe and
 * `ipcRenderer` does not accumulate listeners across re-renders.
 *
 * Frames do **not** cross here. A `MessagePort` is not cloneable, so it cannot
 * pass through `contextBridge`; it is posted into the main world by
 * `preload/index.ts` with `window.postMessage` and the port in the transfer
 * list, which is the documented route and the one that keeps the frame a move
 * rather than a copy (ADR-0178).
 */
import { ipcRenderer } from 'electron'

import { IPC_CHANNELS } from '@shared/ipc-channels'
import type { PlayerEvent } from '@shared/protocol'

export const playerApi = {
  onEvent: (listener: (event: PlayerEvent) => void): (() => void) => {
    const handler = (_e: unknown, event: PlayerEvent): void => listener(event)
    ipcRenderer.on(IPC_CHANNELS.PLAYER_EVENT, handler)
    return () => {
      ipcRenderer.off(IPC_CHANNELS.PLAYER_EVENT, handler)
    }
  },
}
