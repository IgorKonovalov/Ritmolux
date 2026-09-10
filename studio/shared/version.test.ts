/**
 * One version, in three files, held equal.
 *
 * `Cargo.toml`'s `[workspace.package] version` is the single source of truth
 * for this project's version (ADR-0005). The studio carries two copies of it:
 * `studio/package.json`, which the packaging scripts override at build time,
 * and `EXPECTED_PLAYER_VERSION`, which they **cannot** — it is compiled into
 * the renderer bundle, so a stale constant ships as a studio that refuses the
 * player packaged inside it. Nothing else notices: the refusal is correct
 * behaviour for a version the studio does not know, and looks identical to a
 * genuinely mismatched bundle.
 *
 * That is exactly what happened between `0.113.0` and `0.115.0` — two closes
 * moved `Cargo.toml` and neither moved these. This test is why it cannot
 * happen a third time: a version bump now fails the studio's own suite until
 * both copies follow.
 */
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { EXPECTED_PLAYER_VERSION } from './protocol'

const ROOT = join(__dirname, '..', '..')

/**
 * `[workspace.package]`'s `version`, read from the manifest rather than parsed
 * with a TOML library the studio does not depend on.
 *
 * Anchored to the section header so a member crate's own `version` line — or a
 * dependency's — cannot be picked up instead.
 */
function workspaceVersion(): string {
  const manifest = readFileSync(join(ROOT, 'Cargo.toml'), 'utf8')
  const section = manifest.indexOf('[workspace.package]')
  if (section === -1) throw new Error('Cargo.toml has no [workspace.package] section')
  const match = /^version\s*=\s*"([^"]+)"/m.exec(manifest.slice(section))
  if (match === null) throw new Error('[workspace.package] declares no version')
  return match[1]
}

function studioVersion(): string {
  const manifest = JSON.parse(readFileSync(join(ROOT, 'studio', 'package.json'), 'utf8')) as {
    version: string
  }
  return manifest.version
}

describe('the version the studio was built against', () => {
  it('reads a manifest that still looks like the one this test was written for', () => {
    // Semver-shaped, so a regex that quietly matched the wrong line fails here
    // rather than passing three empty strings against each other.
    expect(workspaceVersion()).toMatch(/^\d+\.\d+\.\d+$/)
  })

  it('is the version the workspace declares', () => {
    // The one that ships wrong when it drifts: this constant is compiled into
    // the bundle and no packaging step rewrites it.
    expect(EXPECTED_PLAYER_VERSION).toBe(workspaceVersion())
  })

  it('is the version the studio package declares', () => {
    expect(studioVersion()).toBe(workspaceVersion())
  })
})
