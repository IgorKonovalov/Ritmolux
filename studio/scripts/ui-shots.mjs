// `npm run ui-shots`: one PNG of every studio view and modal, at the window's
// default size and at 1280x800, under `target/ui-audit/` (gitignored).
//
// The same file runs twice. Under Node it is the orchestrator: it finds a built
// player, lays out a scratch home and a fixture preset directory, and relaunches
// itself under Electron. Under Electron it is the entry point: it loads the
// studio's own built main process unchanged, waits for the window it opens, and
// walks the states below with `webContents.executeJavaScript`, saving each with
// `capturePage`. Nothing in `electron/` knows this exists.
//
// The walk drives the page the way a person would - clicking a tab by its text,
// moving a slider - so a state it cannot reach is a failure with a name, never a
// PNG of the wrong thing. The captures use the machine's fonts and a live
// player, so they are for looking at and are never compared.
//
//   npm run ui-shots [-- --player <path>] [--out <dir>] [--mode windowed|windowless]
import { execFileSync, spawn } from 'node:child_process'
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { createRequire } from 'node:module'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const HERE = fileURLToPath(import.meta.url)
const STUDIO = resolve(dirname(HERE), '..')
const REPO = resolve(STUDIO, '..')

/** The fixture's good preset: a shipped one with constants, expressions, a palette and a `[hold]`. */
const FIXTURE_PRESET = join(REPO, 'presets', 'curve_phosphor.toml')
/** The name it reports, which is what the walk waits for. */
const FIXTURE_NAME = 'Phosphor'

/**
 * Two presets the player reports as problems, so the banner offers the list and
 * the problems modal has more than one row. One fails to parse (an error, with a
 * line); one binds a parameter no system declares (a warning, with none).
 */
const BROKEN_PRESETS = {
  'zz_broken_syntax.toml':
    'system = "parametric_curve"\nname = "Broken Syntax"\n\n[params\nscale = "1"\n',
  'zz_unknown_param.toml':
    'system = "parametric_curve"\nname = "Unknown Param"\n\n[curve]\nfamily = "lissajous"\n\n[params]\nnot_a_param = "1"\n',
}

/** The sizes captured, in order. `null` is the size `createWindow` opens at. */
const SIZES = [null, { width: 1280, height: 800 }]

function argValue(argv, flag) {
  const at = argv.indexOf(flag)
  return at === -1 ? undefined : argv[at + 1]
}

if (process.versions.electron === undefined) {
  await orchestrate()
} else {
  // Not awaited: Electron holds `ready` until an ESM entry has finished
  // evaluating, and the walk waits for a window that `ready` opens.
  capture().catch((error) => {
    console.error(error)
    process.exit(1)
  })
}

// ---------------------------------------------------------------------------
// Under Node: the scratch layout and the relaunch.

/** The built `ritmolux`, asked of cargo the way `electron/testing/player.ts` asks. */
function builtPlayer() {
  let target = process.env.CARGO_TARGET_DIR
  if (target === undefined || target === '') {
    const stdout = execFileSync('cargo', ['metadata', '--format-version', '1', '--no-deps'], {
      cwd: STUDIO,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'inherit'],
      env: { ...process.env, RUSTUP_AUTO_INSTALL: '0' },
      maxBuffer: 64 * 1024 * 1024,
    })
    target = JSON.parse(stdout).target_directory
  }
  const name = process.platform === 'win32' ? 'ritmolux.exe' : 'ritmolux'
  for (const profile of ['release', 'debug']) {
    const candidate = join(resolve(target), profile, name)
    if (existsSync(candidate)) return candidate
  }
  return undefined
}

async function orchestrate() {
  const argv = process.argv.slice(2)
  const out = resolve(argValue(argv, '--out') ?? join(STUDIO, 'target', 'ui-audit'))
  const player = argValue(argv, '--player') ?? builtPlayer()
  if (player === undefined || !existsSync(player)) {
    console.error(
      '[ui-shots] no built player: run `cargo build -p standalone` first, or pass --player <path>',
    )
    process.exit(1)
  }
  // Windowless unless asked: a windowed player opens a show window on whatever
  // desktop the run starts on, over the studio it is being captured from. The
  // two modes differ in the studio only in the Settings view's radio and note.
  const mode = argValue(argv, '--mode') ?? 'windowless'
  if (mode !== 'windowed' && mode !== 'windowless') {
    console.error(`[ui-shots] --mode is windowed or windowless, not ${mode}`)
    process.exit(1)
  }
  if (!existsSync(join(STUDIO, 'dist', 'main', 'index.cjs'))) {
    console.error('[ui-shots] no built studio: run `npm run build` first')
    process.exit(1)
  }

  // Scratch, rebuilt every run: the studio's settings, the player's own data
  // directory, and the preset directory it watches. A fork the walk triggers is
  // cancelled, but nothing here would reach the user's files if it were not.
  const scratch = join(out, 'scratch')
  rmSync(scratch, { recursive: true, force: true })
  const userData = join(scratch, 'studio')
  const playerData = join(scratch, 'player')
  const presets = join(scratch, 'presets')
  for (const dir of [userData, playerData, presets]) mkdirSync(dir, { recursive: true })
  writeFileSync(
    join(userData, 'settings.json'),
    `${JSON.stringify({ playerPath: player, playerMode: mode }, null, 2)}\n`,
  )
  copyFileSync(FIXTURE_PRESET, join(presets, 'curve_phosphor.toml'))
  for (const [file, text] of Object.entries(BROKEN_PRESETS))
    writeFileSync(join(presets, file), text)

  const electron = createRequire(import.meta.url)('electron')
  console.log(`[ui-shots] player ${player}`)
  console.log(`[ui-shots] writing ${out}`)
  // Under Wayland a surface the compositor is not showing - another workspace,
  // a covered window - gets no frame callbacks, so the page stops painting and
  // `capturePage` keeps returning its last frame. Through XWayland Chromium
  // paints on its own clock, which is what an unattended walk needs.
  const x11 =
    process.platform === 'linux' &&
    process.env.WAYLAND_DISPLAY !== undefined &&
    process.env.DISPLAY !== undefined
      ? ['--ozone-platform=x11']
      : []
  const child = spawn(electron, [...x11, HERE, '--out', out, '--user-data', userData], {
    cwd: STUDIO,
    stdio: 'inherit',
    env: {
      ...process.env,
      RLX_PRESET_DIR: presets,
      // The player's config, marks and thumbnail cache land in scratch rather
      // than in the per-user directory. macOS has no variable for it, so there
      // the player reads and writes the real one.
      XDG_DATA_HOME: playerData,
      ...(process.platform === 'win32' ? { APPDATA: playerData } : {}),
    },
  })
  // A walk that stalls past every step's own timeout is killed rather than left
  // holding a window and a player.
  const guard = setTimeout(() => {
    console.error('[ui-shots] the walk did not finish in 5 minutes; stopping it')
    child.kill()
  }, 300_000)
  const code = await new Promise((done) => child.on('exit', (exitCode) => done(exitCode ?? 1)))
  clearTimeout(guard)
  process.exit(code)
}

// ---------------------------------------------------------------------------
// Under Electron: the studio's main, and the walk over its window.

const sleep = (ms) => new Promise((done) => setTimeout(done, ms))

/** Settle time after a step, for React, CodeMirror and a few preview frames. */
const SETTLE_MS = 800

async function capture() {
  const { app, BrowserWindow } = await import('electron')
  const argv = process.argv
  const out = argValue(argv, '--out')
  const userData = argValue(argv, '--user-data')
  if (out === undefined || userData === undefined) {
    console.error('[ui-shots] run through `npm run ui-shots`, not under Electron directly')
    process.exit(2)
  }

  // Launched with this file as the entry, Electron finds no package.json beside
  // it and would report its own name and version; the Settings view shows the
  // studio's, so both are put back before the studio's main reads them.
  const pkg = JSON.parse(readFileSync(join(STUDIO, 'package.json'), 'utf8'))
  app.setName('ritmolux-studio')
  app.getVersion = () => pkg.version
  app.setPath('userData', userData)
  // The player's show window opens over the studio's, and Chromium stops
  // compositing a window it believes is covered: `capturePage` then returns the
  // last frame it did composite, which is the previous state's.
  app.commandLine.appendSwitch('disable-backgrounding-occluded-windows')
  app.commandLine.appendSwitch('disable-renderer-backgrounding')

  // Fired inside the constructor, so before the window first maps: pinning its
  // size here is what a tiling window manager reads to float it at that size
  // rather than tile it, and without it the size moves mid-walk as the player's
  // own window opens beside it.
  const windowReady = new Promise((done) =>
    app.on('browser-window-created', (_event, window) => {
      const [width, height] = window.getSize()
      pin(window, { width, height })
      window.webContents.setBackgroundThrottling(false)
      done(window)
    }),
  )
  createRequire(import.meta.url)(join(STUDIO, 'dist', 'main', 'index.cjs'))

  const window = await windowReady
  let failed = false
  try {
    await walk(window, out)
  } catch (error) {
    failed = true
    console.error(`[ui-shots] ${error instanceof Error ? error.message : String(error)}`)
  }
  for (const open of BrowserWindow.getAllWindows()) open.destroy()
  app.exit(failed ? 1 : 0)
}

/** Hold the window at exactly `size`, the way `createWindow` sizes it. */
function pin(window, size) {
  window.setMinimumSize(1, 1)
  window.setMaximumSize(size.width, size.height)
  window.setMinimumSize(size.width, size.height)
  window.setSize(size.width, size.height)
}

/** Evaluate `source` in the page and return its value. */
function page(window, source) {
  return window.webContents.executeJavaScript(`(() => { ${source} })()`, true)
}

/**
 * Ask for a repaint and wait until the page has produced two frames since, so
 * the DOM the step just changed is what `capturePage` reads rather than the
 * frame before it.
 */
async function painted(window) {
  window.webContents.invalidate()
  const framed = await page(
    window,
    `return new Promise((done) => {
       const stall = setTimeout(() => done(false), 3000)
       requestAnimationFrame(() => requestAnimationFrame(() => { clearTimeout(stall); done(true) }))
     })`,
  )
  if (!framed)
    console.warn('[ui-shots] the page produced no frame in 3 s; the capture may be stale')
}

/** Poll `source` in the page until it returns true. */
async function until(window, what, source, timeoutMs = 60_000) {
  const start = Date.now()
  for (;;) {
    if (await page(window, source)) return
    if (Date.now() - start > timeoutMs) throw new Error(`timed out waiting for ${what}`)
    await sleep(250)
  }
}

/** Click the first `selector` whose text is `text`; a miss is a named failure. */
async function click(window, selector, text) {
  const hit = await page(
    window,
    `const el = [...document.querySelectorAll(${JSON.stringify(selector)})]
       .find((node) => node.textContent.trim() === ${JSON.stringify(text)})
     if (el === undefined) return false
     el.click()
     return true`,
  )
  if (!hit) throw new Error(`no ${selector} reading "${text}"`)
}

const tab = (name) => (window) => click(window, 'button[role=tab]', name)

/**
 * The states, each a way in from the resting Editor-on-parameters window and
 * a way back to it. Order matters only in that every `leave` restores that
 * resting state.
 */
const STATES = [
  { name: 'editor-parameters', enter: tab('parameters') },
  { name: 'editor-structure', enter: tab('structure'), leave: tab('parameters') },
  { name: 'editor-palette', enter: tab('palette'), leave: tab('parameters') },
  { name: 'editor-file', enter: tab('file'), leave: tab('parameters') },
  { name: 'library', enter: tab('library'), leave: tab('parameters') },
  {
    name: 'settings',
    enter: (window) => click(window, 'header button', 'settings'),
    leave: (window) => click(window, 'section[aria-label="Studio settings"] button', 'close'),
  },
  {
    name: 'problems',
    enter: async (window) => {
      const hit = await page(
        window,
        `const el = [...document.querySelectorAll('button')]
           .find((node) => /^\\d+ problems$/.test(node.textContent.trim()))
         if (el === undefined) return false
         el.click()
         return true`,
      )
      if (!hit) throw new Error('no "N problems" button: the player reported fewer than two')
    },
    leave: (window) => click(window, '[role=dialog][aria-label=problems] button', 'close'),
  },
  {
    // The first gesture against a preset the studio did not create holds a
    // fork (ADR-0189): move the first live slider to its other end and release.
    name: 'fork-prompt',
    enter: async (window) => {
      await tab('parameters')(window)
      const moved = await page(
        window,
        `const input = document.querySelector('input[type=range]:not([disabled])')
         if (input === null) return false
         const next = Number(input.value) === Number(input.max) ? input.min : input.max
         Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value').set.call(input, next)
         input.dispatchEvent(new Event('input', { bubbles: true }))
         return true`,
      )
      if (!moved) throw new Error('no live slider on the parameters tab')
      await sleep(200)
      // The release is its own event, after React has re-rendered with the
      // moved value, because the commit reads the value from that render.
      await page(
        window,
        `document.querySelector('input[type=range]:not([disabled])')
           .dispatchEvent(new KeyboardEvent('keyup', { bubbles: true }))
         return true`,
      )
      await until(
        window,
        'the fork prompt',
        `return document.querySelector('input[aria-label="save a copy as"]') !== null`,
        5000,
      )
    },
    leave: (window) => click(window, 'form button', 'cancel'),
  },
]

async function walk(window, out) {
  // Resting state: the fixture preset is on screen, its file is open in the
  // editor, and the preview has a stream to paint.
  await until(
    window,
    `the player to report ${FIXTURE_NAME}`,
    `return document.querySelector('header')?.textContent.includes(${JSON.stringify(FIXTURE_NAME)}) === true`,
  )
  await until(
    window,
    'the preview stream',
    `return /\\d+x\\d+ @ \\S+ (rgba8|bgra8)/.test(document.body.textContent)`,
  )
  await until(
    window,
    'the preset file to open',
    `return document.body.textContent.includes('curve_phosphor.toml')`,
  )
  await sleep(2000)

  const [defaultWidth, defaultHeight] = window.getSize()
  for (const size of SIZES) {
    const target = size ?? { width: defaultWidth, height: defaultHeight }
    if (size !== null) {
      pin(window, size)
      await sleep(SETTLE_MS)
    }
    const label = size === null ? 'default' : `${size.width}x${size.height}`
    const dir = join(out, label)
    mkdirSync(dir, { recursive: true })
    for (const state of STATES) {
      await state.enter(window)
      await sleep(SETTLE_MS)
      await painted(window)
      // A window manager may still refuse the size; the capture is kept and the
      // refusal is said, because a PNG at the wrong size is still worth reading.
      const [width, height] = window.getSize()
      const image = await window.webContents.capturePage()
      if (image.isEmpty()) throw new Error(`${label}/${state.name}: capturePage returned nothing`)
      writeFileSync(join(dir, `${state.name}.png`), image.toPNG())
      const shot = image.getSize()
      const off =
        width === target.width && height === target.height
          ? ''
          : ` (window ${width}x${height}, not ${target.width}x${target.height})`
      console.log(`[ui-shots] ${label}/${state.name}.png ${shot.width}x${shot.height} px${off}`)
      if (state.leave !== undefined) await state.leave(window)
      await sleep(200)
    }
  }
}
