/**
 * Where the player binary is, in ADR-0178's order: bundled, then a path from
 * settings, then `PATH`.
 *
 * The order is the contract. A tester receives one zip with the player inside
 * it and must not need to configure anything, so the bundled copy wins; a
 * developer running an unpackaged studio against a build of their own names it
 * in settings; and `PATH` is the last resort that makes a checkout work with
 * the player already installed.
 *
 * Written as a pure function of its inputs so it can be tested without an
 * Electron app object or a real filesystem — the caller supplies the roots and
 * the existence predicate.
 */
import { join } from 'node:path'
import { delimiter } from 'node:path'

/** The executable's name on this platform. */
export function playerExecutableName(platform: NodeJS.Platform = process.platform): string {
  return platform === 'win32' ? 'ritmolux.exe' : 'ritmolux'
}

export interface ResolveInputs {
  /** `process.resourcesPath` in a packaged app; `undefined` when unpackaged. */
  resourcesPath?: string
  /** `playerPath` read from the studio's settings file, if it set one. */
  settingsPath?: string
  /** The raw `PATH` environment variable. */
  pathEnv?: string
  platform?: NodeJS.Platform
  /** True when the path names an existing executable file. */
  exists: (candidate: string) => boolean
}

export type PlayerSource = 'bundled' | 'settings' | 'path'

export interface ResolvedPlayer {
  path: string
  source: PlayerSource
}

/** The roots searched, in order, with the source each one would report. */
export function playerCandidates(inputs: ResolveInputs): ResolvedPlayer[] {
  const exe = playerExecutableName(inputs.platform)
  const candidates: ResolvedPlayer[] = []
  if (inputs.resourcesPath !== undefined) {
    // Where Phase 6's `extraResources` entry puts it; the studio zip's layout.
    candidates.push({ path: join(inputs.resourcesPath, 'player', exe), source: 'bundled' })
  }
  if (inputs.settingsPath !== undefined && inputs.settingsPath.length > 0) {
    candidates.push({ path: inputs.settingsPath, source: 'settings' })
  }
  for (const dir of (inputs.pathEnv ?? '').split(delimiter)) {
    if (dir.length > 0) candidates.push({ path: join(dir, exe), source: 'path' })
  }
  return candidates
}

/** The first candidate that exists, or `undefined` when none does. */
export function resolvePlayer(inputs: ResolveInputs): ResolvedPlayer | undefined {
  return playerCandidates(inputs).find((candidate) => inputs.exists(candidate.path))
}
