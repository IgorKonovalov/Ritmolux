/**
 * Where the built player is, for the tests that spawn it or read its schema.
 *
 * Test-only: nothing under `main.ts` imports this file, so esbuild never
 * bundles it. It lives under `electron/` rather than `shared/` because it
 * needs Node, and `tsconfig.renderer.json` type-checks every file in `shared/`
 * without Node's types.
 *
 * The target directory is **asked of cargo**, never assumed to be `<repo>/target`:
 * `CARGO_TARGET_DIR` when it is set, else `cargo metadata`'s `target_directory`,
 * which also honours a `[build] target-dir` in any ancestor's cargo config.
 * Within it `release` is preferred to `debug`.
 *
 * Every way of not finding a player is a reason string, not a throw, so a
 * caller skips with a notice (ADR-0016's shape) or falls back to a committed
 * snapshot. The answer is cached per module instance, which under Vitest is
 * once per test file.
 */
import { execFileSync } from 'node:child_process'
import { existsSync } from 'node:fs'
import { join, resolve } from 'node:path'

/** The studio's own directory; `cargo metadata` finds the workspace from here. */
const STUDIO = join(__dirname, '..', '..')

/** A built player's path, or why there is none. */
export type BuiltPlayer =
  | { path: string; missing?: undefined }
  | { path?: undefined; missing: string }

let cachedTarget: { dir: string } | { missing: string } | undefined

function targetDirectory(): { dir: string } | { missing: string } {
  if (cachedTarget !== undefined) return cachedTarget
  cachedTarget = askCargo()
  return cachedTarget
}

function askCargo(): { dir: string } | { missing: string } {
  const redirected = process.env.CARGO_TARGET_DIR
  // Cargo resolves a relative `CARGO_TARGET_DIR` against its working directory,
  // which for a spawned cargo would be this process's.
  if (redirected !== undefined && redirected !== '') return { dir: resolve(redirected) }

  let stdout: string
  try {
    stdout = execFileSync('cargo', ['metadata', '--format-version', '1', '--no-deps'], {
      cwd: STUDIO,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'pipe'],
      // A pinned toolchain that is not installed would otherwise be downloaded
      // just to answer where the target directory is; failing is the cheaper
      // answer, and it reads as "no built player" below.
      env: { ...process.env, RUSTUP_AUTO_INSTALL: '0' },
      maxBuffer: 64 * 1024 * 1024,
      timeout: 120_000,
    })
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
      return { missing: 'cargo is not on PATH, so there is no target directory to look in' }
    }
    const stderr = String((error as { stderr?: unknown }).stderr ?? '').trim()
    const first = stderr.split('\n')[0] ?? ''
    return { missing: `cargo metadata failed${first === '' ? '' : `: ${first}`}` }
  }

  const parsed: unknown = JSON.parse(stdout)
  const dir = (parsed as { target_directory?: unknown }).target_directory
  if (typeof dir !== 'string' || dir === '') {
    return { missing: 'cargo metadata named no target_directory' }
  }
  return { dir }
}

/** The built `ritmolux`, `release` before `debug`, in the directory cargo names. */
export function builtPlayer(): BuiltPlayer {
  const target = targetDirectory()
  if ('missing' in target) return { missing: target.missing }
  const name = process.platform === 'win32' ? 'ritmolux.exe' : 'ritmolux'
  for (const profile of ['release', 'debug']) {
    const candidate = join(target.dir, profile, name)
    if (existsSync(candidate)) return { path: candidate }
  }
  return { missing: `no built ritmolux in ${target.dir}` }
}
