/**
 * The clip render's OS surface: two dialogs, the job, and the `render` key of
 * the settings file (ADR-0262).
 *
 * Not domain traffic — nothing here reaches the running player or its control
 * socket. The rules live in `render/service.ts`; this file is the Electron half
 * of it: the dialogs whose answers become granted paths, the push channel the
 * job reports on, and the power-save blocker a job holds.
 */
import {
  dialog,
  ipcMain,
  powerSaveBlocker,
  type BrowserWindow,
  type OpenDialogOptions,
  type SaveDialogOptions,
} from 'electron'

import { IPC_CHANNELS } from '@shared/ipc-channels'
import type { RenderEvent, RenderResult } from '@shared/render'

import type { RenderService } from '../render/service'

/** The formats offered in the track dialog; ffmpeg reads more, and "All" lets them through. */
const AUDIO_EXTENSIONS = ['mp3', 'flac', 'wav', 'ogg', 'opus', 'm4a', 'aac', 'aiff', 'aif']

/** Push one render event to the window, if it is still there. */
export function renderEmitter(window: () => BrowserWindow | undefined): (event: RenderEvent) => void {
  return (event) => {
    const target = window()
    if (target !== undefined && !target.isDestroyed()) {
      target.webContents.send(IPC_CHANNELS.RENDER_EVENT, event)
    }
  }
}

/** `prevent-app-suspension` for as long as the returned release is not called. */
export function stayAwake(): () => void {
  const id = powerSaveBlocker.start('prevent-app-suspension')
  return () => {
    if (powerSaveBlocker.isStarted(id)) powerSaveBlocker.stop(id)
  }
}

export function registerRenderHandlers(
  service: RenderService,
  window: () => BrowserWindow | undefined,
  setRenderSettings: (next: Record<string, string | null>) => void,
): void {
  ipcMain.handle(IPC_CHANNELS.RENDER_PICK_AUDIO, async (): Promise<string | null> => {
    const parent = window()
    const options: OpenDialogOptions = {
      title: 'Choose a track to render',
      properties: ['openFile'],
      filters: [
        { name: 'Audio', extensions: AUDIO_EXTENSIONS },
        { name: 'All files', extensions: ['*'] },
      ],
    }
    const answer = parent ? await dialog.showOpenDialog(parent, options) : await dialog.showOpenDialog(options)
    const path = answer.filePaths[0]
    return answer.canceled || path === undefined ? null : service.grant(path)
  })

  ipcMain.handle(IPC_CHANNELS.RENDER_PREPARE, (_event, source: unknown, fps: unknown) =>
    service.prepare(source, fps),
  )

  ipcMain.handle(IPC_CHANNELS.RENDER_SUGGEST_OUTPUT, (_event, source: unknown, preset: unknown) =>
    service.suggestOutput(source, preset),
  )

  ipcMain.handle(
    IPC_CHANNELS.RENDER_PICK_OUTPUT,
    async (_event, suggested: unknown): Promise<string | null> => {
      const parent = window()
      const options: SaveDialogOptions = {
        title: 'Render the clip to',
        defaultPath: typeof suggested === 'string' ? suggested : undefined,
        filters: [{ name: 'MP4 video', extensions: ['mp4'] }],
      }
      const answer = parent
        ? await dialog.showSaveDialog(parent, options)
        : await dialog.showSaveDialog(options)
      return answer.canceled || answer.filePath === undefined || answer.filePath === ''
        ? null
        : service.grant(answer.filePath)
    },
  )

  ipcMain.handle(IPC_CHANNELS.RENDER_START, (_event, request: unknown) => service.start(request))

  ipcMain.handle(IPC_CHANNELS.RENDER_CANCEL, () => {
    service.cancel()
  })

  ipcMain.handle(
    IPC_CHANNELS.RENDER_SET_SETTINGS,
    (_event, next: unknown): RenderResult<null> => {
      // Validated here rather than trusted: the values are written into a file
      // every launch reads, and they name programs main will run.
      if (typeof next !== 'object' || next === null || Array.isArray(next)) {
        return { ok: false, reason: 'the render settings are an object' }
      }
      const known = ['ffmpegPath', 'outputDir']
      const values: Record<string, string | null> = {}
      for (const [key, value] of Object.entries(next as Record<string, unknown>)) {
        if (!known.includes(key)) return { ok: false, reason: `\`render.${key}\` is not a setting` }
        if (value !== null && typeof value !== 'string') {
          return { ok: false, reason: `\`render.${key}\` is a path` }
        }
        values[key] = value
      }
      try {
        setRenderSettings(values)
        return { ok: true, value: null }
      } catch (error) {
        return { ok: false, reason: (error as Error).message }
      }
    },
  )
}
