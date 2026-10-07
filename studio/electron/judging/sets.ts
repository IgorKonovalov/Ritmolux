/**
 * What a judging set is drawn from, and which files a pick names (ADR-0267).
 *
 * Read from `judging.sourceDir` and nothing else: the source directory is the
 * user's one choice, and a set within it is a pick made per session. Only two
 * levels are read, the top and `proposed/`, because those are the two places a
 * preset lives in a checkout and the player's own `read_dir` is not recursive.
 */
import { existsSync, readdirSync, statSync } from 'node:fs'
import { basename, isAbsolute, join, relative, resolve, sep } from 'node:path'

import type { SetPick, SourceListing } from '@shared/judging'

/** The subdirectory that holds drafts awaiting a verdict. */
export const PROPOSED = 'proposed'

/** The `*.toml` file names directly inside `dir`, sorted; none when it is unreadable. */
function tomlFiles(dir: string): string[] {
  try {
    return readdirSync(dir, { withFileTypes: true })
      .filter((entry) => entry.isFile() && entry.name.toLowerCase().endsWith('.toml'))
      .map((entry) => entry.name)
      .sort()
  } catch {
    return []
  }
}

/** A file name's stem, `.toml` dropped. */
export function stemOf(file: string): string {
  return basename(file).replace(/\.toml$/i, '')
}

/** The family a stem belongs to, or `undefined` when it has no `_`. */
function familyOf(stem: string): string | undefined {
  const at = stem.indexOf('_')
  return at > 0 ? stem.slice(0, at) : undefined
}

function isDirectory(path: string): boolean {
  try {
    return statSync(path).isDirectory()
  } catch {
    return false
  }
}

export function listSets(dir: string): SourceListing {
  const top = tomlFiles(dir)
  const families = [
    ...new Set(top.map((file) => familyOf(stemOf(file))).filter((f) => f !== undefined)),
  ].sort()
  const proposed = isDirectory(join(dir, PROPOSED))
  const drafts = proposed ? tomlFiles(join(dir, PROPOSED)).map((file) => `${PROPOSED}/${file}`) : []
  return { dir, families, proposed, files: [...top, ...drafts] }
}

/**
 * The absolute source paths `pick` names, or why it names none.
 *
 * A listed file must be a `.toml` inside `dir` that exists: the list arrives
 * from the renderer, and a path that walks out of the source directory is
 * refused rather than copied. Two files with one file name cannot both be
 * copied into one flat directory, so that is refused too.
 */
export function resolveSet(dir: string, pick: SetPick): string[] | { refused: string } {
  let files: string[]
  switch (pick.kind) {
    case 'family':
      files = tomlFiles(dir)
        .filter((file) => familyOf(stemOf(file)) === pick.prefix)
        .map((file) => join(dir, file))
      break
    case 'proposed':
      files = tomlFiles(join(dir, PROPOSED)).map((file) => join(dir, PROPOSED, file))
      break
    case 'list': {
      const root = resolve(dir)
      files = []
      for (const name of pick.files) {
        const full = resolve(root, name)
        const inside = relative(root, full)
        if (inside === '..' || inside.startsWith(`..${sep}`) || isAbsolute(inside)) {
          return { refused: `${name} is not inside the source directory` }
        }
        if (!full.toLowerCase().endsWith('.toml') || !existsSync(full)) {
          return { refused: `${name} is not a preset in the source directory` }
        }
        files.push(full)
      }
    }
  }
  if (files.length === 0) return { refused: `the set ${describe(pick)} holds no presets` }
  const seen = new Map<string, string>()
  for (const file of files) {
    const name = basename(file)
    const earlier = seen.get(name)
    if (earlier !== undefined) {
      return { refused: `${earlier} and ${file} share the file name ${name}` }
    }
    seen.set(name, file)
  }
  return files
}

function describe(pick: SetPick): string {
  if (pick.kind === 'family') return `\`${pick.prefix}_*\``
  return pick.kind === 'proposed' ? `\`${PROPOSED}/\`` : 'listed'
}

/**
 * The top-level `name = "..."` of a preset, or `null` when it has none.
 *
 * Read off the lines before the first table header, where the loader reads it.
 * Only a basic string without escapes is read: a name that needs an escape is
 * not one this reads back exactly, and a guessed name would fold the wrong
 * preset's marks.
 */
export function readDisplayName(text: string): string | null {
  for (const raw of text.split('\n')) {
    const line = raw.trim()
    if (line.startsWith('[')) return null
    const match = line.match(/^name\s*=\s*"([^"\\]*)"\s*(?:#.*)?$/)
    if (match !== null) return match[1] === '' ? null : match[1]
  }
  return null
}
