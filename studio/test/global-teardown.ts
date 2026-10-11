import { rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { dirname, resolve } from 'node:path'
import type { TestProject } from 'vitest/node'

/**
 * Removes the directory vitest writes its transformed modules to.
 *
 * The `Vitest` instance names `join(os.tmpdir(), nanoid())` at construction and
 * its module fetcher writes one subdirectory per vite environment under it
 * (`ssr/`, `client/`). A single-project run shares that fetcher, and close
 * removes only the project's own, never-written `tmpDir`, so the shared one
 * outlives every run. No option places it: `os.tmpdir()` is read before the
 * config loads. The path is the internal `_tmpDir`; the teardown removes it only
 * while it is a direct child of `os.tmpdir()`, so a renamed or moved field
 * removes nothing.
 */
export default function setup(project: TestProject): () => Promise<void> {
  const dir = (project.vitest as unknown as { _tmpDir?: unknown })._tmpDir
  return async () => {
    if (typeof dir !== 'string' || dirname(resolve(dir)) !== resolve(tmpdir())) return
    await rm(dir, { recursive: true, force: true })
  }
}
