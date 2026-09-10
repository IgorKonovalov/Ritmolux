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

  /** The studio's own version, the player path it resolved, and its settings. */
  APP_GET_INFO: 'app:get-info',
  /**
   * Write the player mode into the studio's settings file (ADR-0186).
   *
   * An OS channel and not a fourth domain one: it carries no message of the
   * control protocol and never reaches the player. What it needs main for is a
   * file in the per-user directory, which the sandboxed renderer cannot open.
   * The mode is read at spawn, so the answer says a relaunch is what applies it.
   */
  APP_SET_PLAYER_MODE: 'app:set-player-mode',
  /**
   * The resolved player's `--schema` document, fetched once and cached.
   *
   * An OS channel and not a fourth domain one: it carries no message of the
   * control protocol. What it needs main for is a child process, which the
   * sandboxed renderer cannot spawn.
   */
  APP_GET_SCHEMA: 'app:get-schema',

  /**
   * A preset file, read as text. The path comes from the player's own `preset`
   * event; main opens it and never resolves one of its own (ADR-0184).
   */
  PRESET_READ: 'preset:read',
  /** A preset file, written atomically. The renderer holds no file handle. */
  PRESET_WRITE: 'preset:write',
  /**
   * A preset file written to a name that does not exist yet, refused if it
   * does.
   *
   * Separate from the write above because the refusal is the whole of it: a
   * fork (ADR-0189) and a new preset both need a name nothing is using, and a
   * write that silently landed on an existing preset is the damage the fork
   * exists to remove. An OS channel like its two neighbours — it carries no
   * message of the control protocol.
   */
  PRESET_CREATE: 'preset:create',
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
