/**
 * Every IPC channel name, as constants. A bare string in a handler or a preload
 * binding is the drift this file exists to stop.
 *
 * Three of these are **domain** channels and there are only three (ADR-0178):
 * they mirror the control protocol and carry no logic of their own. A fourth is
 * a widening of the protocol, which is settled in the spec before it is code.
 * The rest are the OS surface — the small set that needs the main process
 * because the renderer is sandboxed.
 */
export const IPC_CHANNELS = {
  /** Renderer to main: one `CtlAction`, encoded to OSC and sent on the wire. */
  PLAYER_CTL: 'player:ctl',
  /** Main to renderer: one `PlayerEvent`, already parsed and validated. */
  PLAYER_EVENT: 'player:event',
  /**
   * Main to renderer: the `MessagePort` the frames arrive on, handed across
   * once. The frames themselves never cross an `ipcRenderer` channel — a
   * structured clone per frame is the cost ADR-0178's transfer avoids.
   */
  PLAYER_FRAME: 'player:frame',

  /** The studio's own version and the player path it resolved. */
  APP_GET_INFO: 'app:get-info',
  /** `shell.openExternal`, the only way a link leaves the window. */
  SHELL_OPEN_EXTERNAL: 'shell:open-external',
} as const

export type IpcChannel = (typeof IPC_CHANNELS)[keyof typeof IPC_CHANNELS]

/**
 * The sentinel the preload posts into the main world alongside the frame port.
 *
 * A `MessagePort` cannot cross `contextBridge` — it is not cloneable — so the
 * documented Electron route is `window.postMessage` with the port in the
 * transfer list, which moves it into the main world without a copy. The
 * renderer matches on this exact string and on `event.source === window`, so a
 * message from anywhere else carries no port it will read.
 */
export const FRAME_PORT_SENTINEL = 'ritmolux:frame-port'
