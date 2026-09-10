/**
 * Reading and writing the file the player is watching.
 *
 * The write is a temporary file plus a rename, because the reader is a poll on
 * `read_dir` with no lock between us: a truncate-and-write leaves a window in
 * which the watcher can read half a document, and half a document is a
 * `preset_error` the author did not cause.
 *
 * **The temporary file must not be named `*.toml`.** The player's watcher
 * counts and stats every `.toml` in the directory, so a temporary one is a
 * roster entry that appears and vanishes, and on the tick where it exists it is
 * either a parse error or a second copy of the preset under a different name.
 */
import { access, rename, readFile, writeFile, unlink } from 'node:fs/promises'
import { dirname, join, basename } from 'node:path'

/** The suffix that keeps a half-written file out of the watcher's glob. */
const TEMP_SUFFIX = '.rlx-tmp'

let counter = 0

export interface PresetFile {
  path: string
  text: string
}

/**
 * Read one preset file as text.
 *
 * The path comes from the player's own `preset` event rather than from anything
 * the studio resolved, so what is opened is what is on screen (ADR-0184).
 */
export async function readPreset(path: string): Promise<PresetFile> {
  return { path, text: await readFile(path, 'utf8') }
}

/**
 * Replace `path`'s contents with `text`, atomically.
 *
 * The temporary file is created **beside** the target: a rename is only atomic
 * within one filesystem, and the system temp directory is routinely on another.
 */
export async function writePresetAtomically(path: string, text: string): Promise<void> {
  counter += 1
  const temp = join(dirname(path), `${basename(path)}.${process.pid}-${counter}${TEMP_SUFFIX}`)
  try {
    await writeFile(temp, text, 'utf8')
    await rename(temp, path)
  } catch (error) {
    // A failed rename leaves the temporary file behind, where it is invisible
    // to the watcher but not to a person looking at their preset directory.
    await unlink(temp).catch(() => undefined)
    throw error
  }
}

/**
 * Write `text` to `path`, refusing a name that is already taken.
 *
 * The refusal is the whole point of the call: a fork (ADR-0189) and a new
 * preset both need a name nothing is using, and one that landed on an existing
 * preset would be the silent overwrite the fork exists to remove.
 *
 * **The check is not atomic against another writer, and cannot portably be.**
 * Node has no no-clobber rename, and `open` with `wx` would claim the name with
 * an empty `.toml` the watcher's next poll reads as a broken preset. The other
 * writer this races is the author's own editor, which is one gesture at a time;
 * the hazard that is real — the watcher reading half a document — is what the
 * temporary file below is for.
 */
export async function createPresetFile(path: string, text: string): Promise<void> {
  const taken = await access(path).then(
    () => true,
    () => false,
  )
  if (taken) throw new Error(`${basename(path)} is already there — choose another name`)
  await writePresetAtomically(path, text)
}
