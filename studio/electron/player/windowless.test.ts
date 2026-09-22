/**
 * `windowless` runs the show with no window, and still hands the studio a
 * picture (Plan 0167 Phase 5).
 *
 * Spawned for real, because the claim is about a process: `playerArgs` says what
 * is asked for and only a run says what arrives. It **skips with a notice**
 * when there is no built player, no audio endpoint or no usable adapter, which
 * is ADR-0016's shape applied to a subprocess and the rule the studio's other
 * spawned test already follows.
 *
 * The no-window half needs something that can list another process's
 * windows. On Windows that is the process's own `MainWindowHandle`. Under
 * Hyprland it is the compositor's client list, `hyprctl clients -j`. A Wayland
 * client cannot list other clients' windows, so on any other Wayland desktop
 * and on macOS that half skips. The run still has to produce whole frames of
 * the geometry it announced, and the argument-vector test beside this one pins
 * the flags it was given.
 */
import { spawn } from 'node:child_process'
import { execFileSync } from 'node:child_process'

import { describe, expect, it } from 'vitest'

import { frameBytes, playerEventSchema, type HealthEvent, type StreamEvent } from '@shared/protocol'

import { builtPlayer } from '../testing/player'
import { playerArgs } from './supervisor'

/**
 * Why this machine cannot be asked which windows a process owns, or
 * `undefined` when it can.
 */
function whyWindowsUnasked(): string | undefined {
  if (process.platform === 'win32') return undefined
  if (process.platform === 'linux' && process.env.HYPRLAND_INSTANCE_SIGNATURE) return undefined
  return process.platform === 'linux'
    ? "a Wayland client cannot list other clients' windows, and only Hyprland is asked here"
    : "only Windows and Hyprland are asked for a process's windows here"
}

/**
 * How many windows the OS or compositor shows for `pid`. It is `undefined`
 * where the question cannot be asked, or where the answer could not be read.
 */
function windowsOwnedBy(pid: number): number | undefined {
  if (whyWindowsUnasked() !== undefined) return undefined
  try {
    if (process.platform === 'win32') {
      const out = execFileSync(
        'powershell',
        ['-NoProfile', '-Command', `(Get-Process -Id ${pid}).MainWindowHandle`],
        { encoding: 'utf8' },
      )
      const handle = Number(out.trim())
      // A process that has gone away, or a shell that answered nothing, must
      // not read as "no window". That would pass this test for the wrong reason.
      if (out.trim() === '' || !Number.isFinite(handle)) return undefined
      return handle === 0 ? 0 : 1
    }
    // Hyprland lists every mapped client, XWayland ones included, with the
    // pid that owns it. A player that opened a window shows up here under its
    // own pid.
    const clients: unknown = JSON.parse(
      execFileSync('hyprctl', ['clients', '-j'], { encoding: 'utf8' }),
    )
    if (!Array.isArray(clients)) return undefined
    return clients.filter(
      (client: unknown) =>
        typeof client === 'object' && client !== null && (client as { pid?: unknown }).pid === pid,
    ).length
  } catch {
    return undefined
  }
}

interface Run {
  stream: StreamEvent | undefined
  health: HealthEvent[]
  stdoutBytes: number
  windows: number | undefined
  stderr: string
}

/** One bounded windowless run, watched until it exits. */
async function windowlessRun(player: string): Promise<Run> {
  const child = spawn(
    player,
    // The mode's own vector, plus the two bounds a test needs: a small frame
    // and an end. Neither is a flag the studio passes. Ninety frames at the
    // pipe's 30 fps is three seconds, so the once-a-second `health` event is
    // emitted more than once.
    [...playerArgs('windowless'), '--size', '160x90', '--frames', '90'],
    {
      env: { ...process.env, APPDATA: '', HOME: '', XDG_DATA_HOME: '' },
      stdio: ['ignore', 'pipe', 'pipe'],
    },
  )

  const run: Run = {
    stream: undefined,
    health: [],
    stdoutBytes: 0,
    windows: undefined,
    stderr: '',
  }
  // A chunk boundary can fall inside a line, so only complete lines are parsed
  // and the tail waits for the next chunk.
  let pending = ''
  child.stdout.on('data', (chunk: Buffer) => {
    run.stdoutBytes += chunk.length
    // Asked once the frames are actually flowing: by then a windowed run's
    // window is up, so a zero here is a fact about this mode and not about
    // being early.
    if (run.windows === undefined && child.pid !== undefined) {
      run.windows = windowsOwnedBy(child.pid)
    }
  })
  child.stderr.setEncoding('utf8')
  child.stderr.on('data', (chunk: string) => {
    run.stderr += chunk
    const lines = (pending + chunk).split('\n')
    pending = lines.pop() ?? ''
    for (const line of lines) {
      if (!line.startsWith('{')) continue
      const parsed = playerEventSchema.safeParse(JSON.parse(line))
      if (!parsed.success) continue
      if (parsed.data.ev === 'stream') run.stream = parsed.data
      if (parsed.data.ev === 'health') run.health.push(parsed.data)
    }
  })

  await new Promise<void>((resolve) => {
    child.on('exit', () => resolve())
    child.on('error', () => resolve())
  })
  return run
}

describe('a windowless player', () => {
  it('opens no window and still fills the pipe', async () => {
    const player = builtPlayer()
    if (player.path === undefined) {
      console.warn(`skipped: ${player.missing}`)
      return
    }
    const run = await windowlessRun(player.path)
    if (
      run.stderr.includes('no audio capture device is available') ||
      (run.stderr.includes('--stream: ') && run.stderr.includes('adapter'))
    ) {
      console.warn('skipped: this machine has no capture endpoint or no usable adapter')
      return
    }

    expect(run.stream, `the run announced no stream:\n${run.stderr}`).toBeDefined()
    if (run.stream === undefined) return
    // The studio shows a picture: whole frames, of exactly the geometry the
    // event named, and nothing left over.
    expect(run.stdoutBytes).toBeGreaterThan(0)
    expect(run.stdoutBytes % frameBytes(run.stream)).toBe(0)

    // The footer's rate is read off `health`, and a windowless run draws
    // through the frame tap rather than a present: the reading has to be the
    // rate it drew at, not the 0.0 of a clock nothing fed.
    const last = run.health.at(-1)
    expect(last, `the run reported no health in three seconds:\n${run.stderr}`).toBeDefined()
    expect(last?.fps ?? 0).toBeGreaterThan(0)

    const unasked = whyWindowsUnasked()
    if (unasked !== undefined) {
      console.warn(`skipped the no-window half: ${unasked}`)
      return
    }
    expect(run.windows, 'the process was gone before it could be asked').toBeDefined()
    expect(run.windows).toBe(0)
  }, 300_000)
})
