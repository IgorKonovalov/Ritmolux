/**
 * The `BrowserWindow` factory and the double Content-Security-Policy.
 *
 * The policy is set twice — as the `<meta>` in `renderer/index.html` and as the
 * response header installed here — because a dev server sends a policy of its
 * own, and the header hook strips any incoming one **case-insensitively** before
 * writing ours. A header named `content-security-policy` and one named
 * `Content-Security-Policy` are the same header to a browser and two different
 * keys to an object, which is the trap this loop exists for.
 *
 * Tighter than the shell this is adapted from in one place: there is no
 * `connect-src` relaxation, because the renderer makes no network call of any
 * kind. Everything it shows arrives over IPC (ADR-0178).
 */
import { BrowserWindow, session, shell } from 'electron'
import { join } from 'node:path'

/** The dev server's origin. Vite is told the same port in `vite.config.ts`. */
export const DEV_SERVER_ORIGIN = 'http://localhost:5273'

export function prodCsp(): string {
  return [
    "default-src 'self'",
    "script-src 'self'",
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' data:",
    // No network of any kind: `connect-src 'none'` is the whole statement.
    "connect-src 'none'",
  ].join('; ')
}

export function devCsp(): string {
  return [
    `default-src 'self' ${DEV_SERVER_ORIGIN}`,
    // The dev server's client is injected inline and its HMR socket is a
    // WebSocket; both are unpackaged-only relaxations.
    `script-src 'self' 'unsafe-inline' ${DEV_SERVER_ORIGIN}`,
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' data:",
    `connect-src 'self' ${DEV_SERVER_ORIGIN} ws://localhost:5273`,
  ].join('; ')
}

/**
 * Install the policy as a response header on the default session, replacing
 * whatever arrived.
 */
export function installCsp(isDev: boolean): void {
  const csp = isDev ? devCsp() : prodCsp()
  session.defaultSession.webRequest.onHeadersReceived((details, callback) => {
    const headers = { ...details.responseHeaders }
    for (const key of Object.keys(headers)) {
      if (key.toLowerCase() === 'content-security-policy') delete headers[key]
    }
    headers['Content-Security-Policy'] = [csp]
    callback({ responseHeaders: headers })
  })
}

export interface CreateWindowOptions {
  preloadPath: string
  /** Set when a dev server is serving the renderer; unset loads the built file. */
  rendererUrl?: string
  rendererFile: string
}

export function createWindow(opts: CreateWindowOptions): BrowserWindow {
  const window = new BrowserWindow({
    width: 1440,
    height: 900,
    minWidth: 960,
    minHeight: 600,
    show: false,
    backgroundColor: '#101014',
    title: 'Ritmolux Studio',
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      preload: opts.preloadPath,
      webSecurity: true,
    },
  })

  window.once('ready-to-show', () => window.show())

  // Nothing opens a second window; a link goes to the OS browser instead.
  window.webContents.setWindowOpenHandler(({ url }) => {
    if (url.startsWith('https://')) void shell.openExternal(url)
    return { action: 'deny' }
  })

  window.webContents.on('will-navigate', (event, url) => {
    const sameOrigin =
      (opts.rendererUrl !== undefined && url.startsWith(opts.rendererUrl)) ||
      url.startsWith('file://')
    if (!sameOrigin) event.preventDefault()
  })

  if (opts.rendererUrl !== undefined) {
    void window.loadURL(opts.rendererUrl)
  } else {
    void window.loadFile(opts.rendererFile)
  }

  return window
}

export function getRendererPaths(): { rendererFile: string; preloadPath: string } {
  return {
    rendererFile: join(__dirname, '..', 'renderer', 'index.html'),
    preloadPath: join(__dirname, '..', 'preload', 'index.cjs'),
  }
}
