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
 * The no-window half is asserted on Windows only, through the process's own
 * `MainWindowHandle`. There is no cross-platform way to ask the OS whether a
 * child opened a window; on the other platforms the run still has to produce
 * whole frames of the geometry it announced, and the flags it was given are
 * pinned by the argument-vector test beside this one.
 */
import { spawn } from 'node:child_process'
import { execFileSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { frameBytes, playerEventSchema, type StreamEvent } from '@shared/protocol'

import { playerArgs } from './supervisor'

const ROOT = join(__dirname, '..', '..', '..')

function builtPlayer(): string | undefined {
  const name = process.platform === 'win32' ? 'ritmolux.exe' : 'ritmolux'
  for (const profile of ['release', 'debug']) {
    const candidate = join(ROOT, 'target', profile, name)
    if (existsSync(candidate)) return candidate
  }
  return undefined
}

/**
 * The window handle the OS has for `pid`, or `undefined` where the question
 * cannot be asked. `0` is "this process has no main window".
 */
function mainWindowHandle(pid: number): number | undefined {
  if (process.platform !== 'win32') return undefined
  const out = execFileSync(
    'powershell',
    ['-NoProfile', '-Command', `(Get-Process -Id ${pid}).MainWindowHandle`],
    { encoding: 'utf8' },
  )
  const value = Number(out.trim())
  // A process that has gone away, or a shell that answered nothing, must not
  // read as "no window" — that would pass this test for the wrong reason.
  return Number.isFinite(value) ? value : undefined
}

interface Run {
  stream: StreamEvent | undefined
  stdoutBytes: number
  handle: number | undefined
  stderr: string
}

/** One bounded windowless run, watched until it exits. */
async function windowlessRun(player: string): Promise<Run> {
  const child = spawn(
    player,
    // The mode's own vector, plus the two bounds a test needs: a small frame
    // and an end. Neither is a flag the studio passes.
    [...playerArgs('windowless'), '--size', '160x90', '--frames', '30'],
    {
      env: { ...process.env, APPDATA: '', HOME: '', XDG_DATA_HOME: '' },
      stdio: ['ignore', 'pipe', 'pipe'],
    },
  )

  const run: Run = { stream: undefined, stdoutBytes: 0, handle: undefined, stderr: '' }
  child.stdout.on('data', (chunk: Buffer) => {
    run.stdoutBytes += chunk.length
    // Asked once the frames are actually flowing: by then a windowed run's
    // window is up, so a zero here is a fact about this mode and not about
    // being early.
    if (run.handle === undefined && child.pid !== undefined) {
      run.handle = mainWindowHandle(child.pid)
    }
  })
  child.stderr.setEncoding('utf8')
  child.stderr.on('data', (chunk: string) => {
    run.stderr += chunk
    for (const line of chunk.split('\n')) {
      if (!line.startsWith('{')) continue
      const parsed = playerEventSchema.safeParse(JSON.parse(line))
      if (parsed.success && parsed.data.ev === 'stream') run.stream = parsed.data
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
    if (player === undefined) {
      console.warn('skipped: no built ritmolux in target/')
      return
    }
    const run = await windowlessRun(player)
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

    if (process.platform !== 'win32') {
      console.warn('skipped the no-window half: only Windows is asked for a window handle here')
      return
    }
    expect(run.handle, 'the process was gone before it could be asked').toBeDefined()
    expect(run.handle).toBe(0)
  }, 300_000)
})
