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
import { rename, readFile, writeFile, unlink } from 'node:fs/promises'
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
