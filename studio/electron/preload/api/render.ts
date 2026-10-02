/**
 * The clip render's side of `window.api` (ADR-0262).
 *
 * Every call resolves rather than rejects, with a reason the view can show. The
 * event binding returns its own cleanup, like the player's.
 */
import { ipcRenderer } from 'electron'

import { IPC_CHANNELS } from '@shared/ipc-channels'
import type {
  PreparedTrack,
  RenderEvent,
  RenderRequest,
  RenderResult,
  RenderSettings,
} from '@shared/render'

/** A `render` settings write: a value sets its key, `null` or `''` clears it. */
export type RenderSettingsPatch = { [K in keyof RenderSettings]?: string | null }

export const renderApi = {
  /** The track dialog; `null` when it was dismissed. */
  pickAudio: (): Promise<string | null> =>
    ipcRenderer.invoke(IPC_CHANNELS.RENDER_PICK_AUDIO) as Promise<string | null>,

  /** Transcode the track and count its bars at `fps`. */
  prepare: (source: string, fps: string): Promise<RenderResult<PreparedTrack>> =>
    ipcRenderer.invoke(IPC_CHANNELS.RENDER_PREPARE, source, fps) as Promise<
      RenderResult<PreparedTrack>
    >,

  /** The default output for this track and preset. */
  suggestOutput: (source: string, preset: string): Promise<RenderResult<string>> =>
    ipcRenderer.invoke(IPC_CHANNELS.RENDER_SUGGEST_OUTPUT, source, preset) as Promise<
      RenderResult<string>
    >,

  /** The save dialog, opened on `suggested`; `null` when it was dismissed. */
  pickOutput: (suggested: string | undefined): Promise<string | null> =>
    ipcRenderer.invoke(IPC_CHANNELS.RENDER_PICK_OUTPUT, suggested) as Promise<string | null>,

  start: (request: RenderRequest): Promise<RenderResult<null>> =>
    ipcRenderer.invoke(IPC_CHANNELS.RENDER_START, request) as Promise<RenderResult<null>>,

  cancel: (): Promise<void> => ipcRenderer.invoke(IPC_CHANNELS.RENDER_CANCEL) as Promise<void>,

  onEvent: (listener: (event: RenderEvent) => void): (() => void) => {
    const handler = (_e: unknown, event: RenderEvent): void => listener(event)
    ipcRenderer.on(IPC_CHANNELS.RENDER_EVENT, handler)
    return () => {
      ipcRenderer.off(IPC_CHANNELS.RENDER_EVENT, handler)
    }
  },

  /** Write the `render` key of `settings.json`. */
  setSettings: (patch: RenderSettingsPatch): Promise<RenderResult<null>> =>
    ipcRenderer.invoke(IPC_CHANNELS.RENDER_SET_SETTINGS, patch) as Promise<RenderResult<null>>,
}
