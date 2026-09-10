/**
 * The frame port, captured the moment it arrives.
 *
 * Main posts it once, on `did-finish-load`, which can be before React has
 * mounted anything. The listener is installed at module scope — that is the
 * point of this file — so the port is held rather than missed, and a component
 * that asks later is handed the one already received.
 */
import { FRAME_PORT_SENTINEL } from '@shared/ipc-channels'

let held: MessagePort | undefined
const waiting: ((port: MessagePort) => void)[] = []

window.addEventListener('message', (event: MessageEvent) => {
  // `event.source === window` is the check that matters: it rejects a message
  // from a frame or another window, so only the preload's hand-off is read.
  if (event.source !== window || event.data !== FRAME_PORT_SENTINEL) return
  const port = event.ports[0]
  if (port === undefined) return
  held = port
  for (const resolve of waiting.splice(0)) resolve(port)
})

/** Call `listener` with the port, now or when it arrives. */
export function onFramePort(listener: (port: MessagePort) => void): () => void {
  if (held !== undefined) {
    listener(held)
    return () => undefined
  }
  waiting.push(listener)
  return () => {
    const at = waiting.indexOf(listener)
    if (at !== -1) waiting.splice(at, 1)
  }
}
