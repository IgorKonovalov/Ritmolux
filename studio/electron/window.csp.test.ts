/**
 * The security defaults and both policy sites are asserted, not trusted
 * (Plan 0159 Phase 1, ADR-0178).
 *
 * The `<meta>` in `index.html` and the header this module builds are two
 * statements of one policy, and a drift between them is invisible at runtime:
 * whichever is stricter silently wins. This reads the file and holds them equal.
 */
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { devCsp, prodCsp } from './window'

const root = join(__dirname, '..')

function metaCsp(): string {
  const html = readFileSync(join(root, 'renderer', 'index.html'), 'utf8')
  const match = html.match(/http-equiv="Content-Security-Policy"\s+content="([^"]+)"/)
  if (match === null) throw new Error('renderer/index.html carries no CSP <meta>')
  return match[1]
}

describe('the production policy', () => {
  const csp = prodCsp()

  it('is the same policy the index.html meta tag states', () => {
    // If this fails, one of the two sites was edited alone.
    expect(metaCsp()).toBe(csp)
  })

  it('admits no inline script', () => {
    const scriptSrc = csp.split('; ').find((d) => d.startsWith('script-src'))
    expect(scriptSrc).toBe("script-src 'self'")
  })

  it('admits no eval, in any directive', () => {
    expect(csp).not.toContain('unsafe-eval')
  })

  it('makes no network reachable, because the renderer makes no request', () => {
    expect(csp).toContain("connect-src 'none'")
  })

  it('admits no remote image host', () => {
    expect(csp.split('; ').find((d) => d.startsWith('img-src'))).toBe("img-src 'self' data:")
  })
})

describe('the development policy', () => {
  const csp = devCsp()

  it('relaxes inline script only for the dev server, and never eval', () => {
    expect(csp).toContain("'unsafe-inline'")
    expect(csp).not.toContain('unsafe-eval')
  })

  it('reaches the dev server and nothing else', () => {
    const connect = csp.split('; ').find((d) => d.startsWith('connect-src'))
    expect(connect).toBe("connect-src 'self' http://localhost:5273 ws://localhost:5273")
  })
})

describe('the window is built with the sandbox on', () => {
  // The factory needs a live Electron app, so the assertion is made against the
  // source: these four lines are the ones an incident would trace back to, and
  // a test that imports `electron` here would assert nothing at all.
  const source = readFileSync(join(root, 'electron', 'window.ts'), 'utf8')

  it('sets contextIsolation, no nodeIntegration, and sandbox on every window', () => {
    expect(source).toContain('contextIsolation: true')
    expect(source).toContain('nodeIntegration: false')
    expect(source).toContain('sandbox: true')
  })

  it('holds the window back until it has something to show', () => {
    expect(source).toContain('show: false')
    expect(source).toContain('ready-to-show')
  })

  it('intercepts navigation and the window-open handler', () => {
    expect(source).toContain('setWindowOpenHandler')
    expect(source).toContain("'will-navigate'")
  })

  it('strips an incoming policy case-insensitively before writing ours', () => {
    expect(source).toContain('toLowerCase() === ')
  })
})
