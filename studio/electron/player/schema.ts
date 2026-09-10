/**
 * The engine's schema document, read once from the resolved player.
 *
 * `ritmolux --schema` prints it and exits, so this is a short child process and
 * not the show's. It runs once per studio launch and the promise is memoized:
 * the document is a property of the binary, and the binary does not change
 * under a running studio.
 *
 * The parse is split from the run so it can be tested without spawning
 * anything — `parseSchemaDocument` is the boundary the renderer's types rest
 * on, and it is where a document from an unexpected build is refused.
 */
import { execFile } from 'node:child_process'

import { schemaDocumentSchema, type SchemaDocument } from '@shared/schema'

/** How long the player gets to print the document before we give up on it. */
const SCHEMA_TIMEOUT_MS = 10_000

/**
 * The document is around 60 KB today and grows with the engine; the cap is
 * generous enough that a bigger one is not a truncated parse, and finite so a
 * binary that streams instead of printing cannot exhaust memory.
 */
const SCHEMA_MAX_BYTES = 8 * 1024 * 1024

export class SchemaError extends Error {}

/**
 * Validate one `--schema` document.
 *
 * A refusal names what was wrong, because the only place this reaches a user is
 * a panel that will not render — and "the schema could not be read" with no
 * reason is a support question rather than a message.
 */
export function parseSchemaDocument(text: string): SchemaDocument {
  let json: unknown
  try {
    json = JSON.parse(text)
  } catch (error) {
    throw new SchemaError(
      `the player printed something that is not JSON: ${(error as Error).message}`,
    )
  }
  const parsed = schemaDocumentSchema.safeParse(json)
  if (!parsed.success) {
    const first = parsed.error.issues[0]
    throw new SchemaError(
      `the schema document is not one this studio reads: ${first.path.join('.')} ${first.message}`,
    )
  }
  return parsed.data
}

/** Run one command and hand back its standard output. */
export type RunSchema = (playerPath: string) => Promise<string>

const runWithExecFile: RunSchema = (playerPath) =>
  new Promise((resolve, reject) => {
    execFile(
      playerPath,
      ['--schema'],
      { timeout: SCHEMA_TIMEOUT_MS, maxBuffer: SCHEMA_MAX_BYTES, windowsHide: true },
      (error, stdout) => {
        if (error) {
          reject(new SchemaError(`could not run the player with --schema: ${error.message}`))
          return
        }
        resolve(stdout)
      },
    )
  })

/**
 * The document, fetched at most once.
 *
 * The **promise** is cached rather than the result, so two callers during the
 * first fetch share one child process instead of racing two. A failure is
 * cached too: a player that cannot print its schema will not start being able
 * to under the same studio, and retrying per keystroke would spawn a process
 * per attempt.
 */
export class SchemaCache {
  private pending: Promise<SchemaDocument> | undefined

  constructor(
    private readonly playerPath: string | undefined,
    private readonly run: RunSchema = runWithExecFile,
  ) {}

  get(): Promise<SchemaDocument> {
    this.pending ??= this.fetch()
    return this.pending
  }

  private async fetch(): Promise<SchemaDocument> {
    if (this.playerPath === undefined) {
      throw new SchemaError('no player was resolved, so there is no schema to read')
    }
    return parseSchemaDocument(await this.run(this.playerPath))
  }
}
