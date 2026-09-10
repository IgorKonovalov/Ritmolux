/**
 * The preset file's side of `window.api`.
 *
 * Both calls answer with a result object rather than throwing: a refusal here
 * is a message the panel shows against the file, and an unhandled rejection
 * crossing `contextBridge` arrives as an Error whose text the user cannot act
 * on.
 */
import { ipcRenderer } from 'electron'

import { IPC_CHANNELS } from '@shared/ipc-channels'

import type { PresetResult } from '../../ipc/presetHandlers'

export const presetApi = {
  read: (path: string): Promise<PresetResult<{ path: string; text: string }>> =>
    ipcRenderer.invoke(IPC_CHANNELS.PRESET_READ, path) as Promise<
      PresetResult<{ path: string; text: string }>
    >,

  /** One atomic write. The watcher's next poll is what makes it visible. */
  write: (path: string, text: string): Promise<PresetResult<null>> =>
    ipcRenderer.invoke(IPC_CHANNELS.PRESET_WRITE, path, text) as Promise<PresetResult<null>>,
}
