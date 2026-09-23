/**
 * `StudioSettings` and `studio/README.md`'s settings table are held equal, both
 * ways (Plan 0217 Phase 4).
 *
 * ADR-0240 makes a file the definition of every setting: a choice the user
 * expects to outlive the window is a key in `settings.json`, and the panel that
 * changes it is an editor of that file. The standalone's half of that rule is
 * carried by types - `SettingsRow::config_path` is an exhaustive match from a
 * menu row to a `config.toml` key - and the cross-language half by
 * `scripts/check-settings-have-files.mjs`. Neither can see whether a studio key
 * is documented, which is this file.
 *
 * The diff runs in **both** directions on purpose. A key with no row is a
 * setting a user can only discover by reading source; a row with no key is a
 * document promising something the studio does not read. Either is a fork.
 *
 * The interface is read as SOURCE TEXT rather than as a value, because a
 * TypeScript interface is erased before anything runs: there is no
 * `Config::default()` here to serialise and walk. That costs the parse below,
 * and it buys a check with nothing to keep in sync - the declaration a
 * developer edits is the one this reads.
 */
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

const SETTINGS = join(__dirname, 'settings.ts')
const README = join(__dirname, '..', 'README.md')

/** The heading the documented roster lives under. */
const HEADING = '## Every setting has a key in `settings.json`'

/**
 * Every property name declared by the `StudioSettings` interface.
 *
 * Reads to the first `}` at column zero, which is where the interface ends in a
 * file the formatter has been over. Comment lines are dropped first so a `:` in
 * prose cannot read as a property.
 */
function declaredKeys(): string[] {
  const text = readFileSync(SETTINGS, 'utf8')
  const start = text.indexOf('export interface StudioSettings {')
  if (start === -1) throw new Error('settings.ts no longer declares StudioSettings')
  const body = text.slice(start).split('\n').slice(1)
  const keys: string[] = []
  for (const line of body) {
    const trimmed = line.trim()
    if (line.startsWith('}')) break
    if (trimmed.startsWith('*') || trimmed.startsWith('//') || trimmed.startsWith('/*')) continue
    const match = trimmed.match(/^(?:readonly\s+)?([A-Za-z_$][\w$]*)\??\s*:/)
    if (match) keys.push(match[1])
  }
  if (keys.length === 0) throw new Error('parsed no keys out of StudioSettings')
  return keys
}

/**
 * The first backticked name in each row of the table under the heading.
 *
 * Splits on plain pipes: the table is written without escaped ones, and the
 * test that a row parses at all is that this finds a code span in its first
 * cell.
 */
function documentedKeys(): string[] {
  const text = readFileSync(README, 'utf8')
  const start = text.indexOf(HEADING)
  if (start === -1) throw new Error(`studio/README.md has no "${HEADING}" section`)
  const keys: string[] = []
  let seenSeparator = false
  for (const line of text.slice(start).split('\n').slice(1)) {
    if (!line.trimStart().startsWith('|')) {
      if (keys.length > 0) break
      continue
    }
    if (/^\|[\s:|-]+\|$/.test(line.trim())) {
      seenSeparator = true
      continue
    }
    if (!seenSeparator) continue
    const first = line.split('|')[1] ?? ''
    const code = first.match(/`([^`]+)`/)
    if (code === null) throw new Error(`no key in backticks in row: ${line.trim()}`)
    keys.push(code[1])
  }
  if (keys.length === 0) throw new Error(`no table found under "${HEADING}"`)
  return keys
}

describe('every studio setting has a key in the file and a row in the document', () => {
  it('documents every key StudioSettings declares', () => {
    const undocumented = declaredKeys().filter((key) => !documentedKeys().includes(key))
    expect(undocumented).toEqual([])
  })

  it('declares every key the document names', () => {
    const unimplemented = documentedKeys().filter((key) => !declaredKeys().includes(key))
    expect(unimplemented).toEqual([])
  })

  it('reads both sides rather than passing on an empty set', () => {
    // A parse that silently found nothing would make the two diffs above pass
    // for the wrong reason, which is the failure mode of a check over source
    // text. Both parsers throw on an empty read; this asserts the keys that
    // exist today are actually in hand.
    expect(declaredKeys()).toContain('playerPath')
    expect(documentedKeys()).toContain('playerMode')
  })
})
