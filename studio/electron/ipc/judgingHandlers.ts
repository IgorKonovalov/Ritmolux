/**
 * The judging session's OS surface (ADR-0267).
 *
 * Not domain traffic — nothing here is a control-protocol message; the marks a
 * session folds arrive on the player's event stream. The rules live in
 * `judging/session.ts`; this file is the Electron half: the directory dialog
 * and the settings write. Every argument the renderer sends is validated in
 * main — here, or by the service it is handed to — never trusted.
 */
import { dialog, ipcMain, type BrowserWindow, type OpenDialogOptions } from 'electron'

import { IPC_CHANNELS } from '@shared/ipc-channels'
import type {
  JudgingResult,
  JudgingState,
  LedgerLine,
  SessionDir,
  SourceListing,
} from '@shared/judging'

import { listSets } from '../judging/sets'
import type { JudgingService } from '../judging/session'

export interface JudgingSurface {
  service: JudgingService
  window: () => BrowserWindow | undefined
  sourceDir: () => string | undefined
  /** Write `judging.sourceDir`; `null` clears it. Throws when the file cannot be written. */
  setSourceDir: (next: string | null) => void
}

export function registerJudgingHandlers(surface: JudgingSurface): void {
  const { service } = surface

  ipcMain.handle(
    IPC_CHANNELS.JUDGING_GET_STATE,
    (): JudgingState => ({ sourceDir: surface.sourceDir() ?? null, session: service.session }),
  )

  const setSource = (next: string | null): JudgingResult<string | null> => {
    try {
      surface.setSourceDir(next)
      return { ok: true, value: surface.sourceDir() ?? null }
    } catch (error) {
      return { ok: false, reason: (error as Error).message }
    }
  }

  ipcMain.handle(
    IPC_CHANNELS.JUDGING_SET_SOURCE_DIR,
    (_event, next: unknown): JudgingResult<string | null> => {
      if (next !== null && typeof next !== 'string') {
        return { ok: false, reason: '`judging.sourceDir` is a path' }
      }
      return setSource(next)
    },
  )

  ipcMain.handle(
    IPC_CHANNELS.JUDGING_PICK_SOURCE_DIR,
    async (): Promise<JudgingResult<string | null> | null> => {
      const parent = surface.window()
      const options: OpenDialogOptions = {
        title: 'Choose the preset directory to judge from',
        defaultPath: surface.sourceDir(),
        properties: ['openDirectory'],
      }
      const answer = parent
        ? await dialog.showOpenDialog(parent, options)
        : await dialog.showOpenDialog(options)
      const path = answer.filePaths[0]
      return answer.canceled || path === undefined ? null : setSource(path)
    },
  )

  ipcMain.handle(IPC_CHANNELS.JUDGING_LIST_SETS, (): JudgingResult<SourceListing> => {
    const dir = surface.sourceDir()
    if (dir === undefined) return { ok: false, reason: 'no source directory is set' }
    return { ok: true, value: listSets(dir) }
  })

  ipcMain.handle(IPC_CHANNELS.JUDGING_START, (_event, pick: unknown) => service.start(pick))
  ipcMain.handle(IPC_CHANNELS.JUDGING_END, () => service.end())
  ipcMain.handle(IPC_CHANNELS.JUDGING_WRITE_BACK, (_event, stems: unknown) =>
    service.writeBack(stems),
  )
  ipcMain.handle(IPC_CHANNELS.JUDGING_LEDGER, (): LedgerLine[] => service.ledger())
  ipcMain.handle(IPC_CHANNELS.JUDGING_LIST_SESSIONS, (): SessionDir[] => service.listSessions())
  ipcMain.handle(IPC_CHANNELS.JUDGING_DELETE_SESSIONS, (_event, runs: unknown) =>
    service.deleteSessions(runs),
  )
}
