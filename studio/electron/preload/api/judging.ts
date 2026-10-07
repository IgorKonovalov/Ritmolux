/**
 * The judging session's side of `window.api` (ADR-0267).
 *
 * Every call resolves rather than rejects, with a reason the view can show.
 * The marks a session folds are not here: they arrive as `marks` events on the
 * player's own binding.
 */
import { ipcRenderer } from 'electron'

import { IPC_CHANNELS } from '@shared/ipc-channels'
import type {
  DeleteResult,
  EndedSession,
  JudgingResult,
  JudgingState,
  LedgerLine,
  SessionDir,
  SessionInfo,
  SetPick,
  SourceListing,
  WriteBackResult,
} from '@shared/judging'

export const judgingApi = {
  getState: (): Promise<JudgingState> =>
    ipcRenderer.invoke(IPC_CHANNELS.JUDGING_GET_STATE) as Promise<JudgingState>,

  /** Write `judging.sourceDir`; `null` clears it. Answers with the value now held. */
  setSourceDir: (path: string | null): Promise<JudgingResult<string | null>> =>
    ipcRenderer.invoke(IPC_CHANNELS.JUDGING_SET_SOURCE_DIR, path) as Promise<
      JudgingResult<string | null>
    >,

  /** The directory dialog, written when chosen; `null` when it was dismissed. */
  pickSourceDir: (): Promise<JudgingResult<string | null> | null> =>
    ipcRenderer.invoke(IPC_CHANNELS.JUDGING_PICK_SOURCE_DIR) as Promise<JudgingResult<
      string | null
    > | null>,

  listSets: (): Promise<JudgingResult<SourceListing>> =>
    ipcRenderer.invoke(IPC_CHANNELS.JUDGING_LIST_SETS) as Promise<JudgingResult<SourceListing>>,

  start: (pick: SetPick): Promise<JudgingResult<SessionInfo>> =>
    ipcRenderer.invoke(IPC_CHANNELS.JUDGING_START, pick) as Promise<JudgingResult<SessionInfo>>,

  end: (): Promise<JudgingResult<EndedSession>> =>
    ipcRenderer.invoke(IPC_CHANNELS.JUDGING_END) as Promise<JudgingResult<EndedSession>>,

  /** Write the named stems of the ended session back over their sources. */
  writeBack: (stems: string[]): Promise<JudgingResult<WriteBackResult>> =>
    ipcRenderer.invoke(IPC_CHANNELS.JUDGING_WRITE_BACK, stems) as Promise<
      JudgingResult<WriteBackResult>
    >,

  ledger: (): Promise<LedgerLine[]> =>
    ipcRenderer.invoke(IPC_CHANNELS.JUDGING_LEDGER) as Promise<LedgerLine[]>,

  listSessions: (): Promise<SessionDir[]> =>
    ipcRenderer.invoke(IPC_CHANNELS.JUDGING_LIST_SESSIONS) as Promise<SessionDir[]>,

  deleteSessions: (runs: string[]): Promise<JudgingResult<DeleteResult>> =>
    ipcRenderer.invoke(IPC_CHANNELS.JUDGING_DELETE_SESSIONS, runs) as Promise<
      JudgingResult<DeleteResult>
    >,
}
