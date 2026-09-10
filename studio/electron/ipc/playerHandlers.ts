/**
 * The renderer's actions, validated and put on the wire.
 *
 * Main validates **every** action it forwards (ADR-0178). The renderer is the
 * only sender today and its types already say the same thing, but the schema is
 * the boundary and a boundary that trusts its caller is not one.
 */
import { ipcMain } from 'electron'

import { IPC_CHANNELS } from '@shared/ipc-channels'
import { ctlActionSchema } from '@shared/protocol'

import type { ControlSender } from '../player/control'

export function registerPlayerHandlers(
  sender: () => ControlSender | undefined,
  onRejected: (reason: string) => void,
): void {
  ipcMain.on(IPC_CHANNELS.PLAYER_CTL, (_event, raw: unknown) => {
    const parsed = ctlActionSchema.safeParse(raw)
    if (!parsed.success) {
      onRejected(parsed.error.issues.map((issue) => issue.message).join('; '))
      return
    }
    sender()?.send(parsed.data)
  })
}
