/**
 * The job file: `<output>.render.json`, written beside the MP4 when a render
 * starts, and read back by **Open job** to restore the whole view (ADR-0262).
 *
 * Written and read only by the studio. It is the request Start sent, with the
 * track under `audio` and a `version` in front, so a later studio can tell a
 * document it reads from one it does not and say so instead of guessing at the
 * fields. A document is parsed at this boundary and never asserted into shape.
 */
import { renameSync, writeFileSync } from 'node:fs'

import { z } from 'zod'

import { neuralSchema, renderRequestSchema, type RenderRequest } from '@shared/render'

/** The one version this studio writes and reads. */
export const JOB_VERSION = 1

const jobSchema = z
  .object({
    version: z.literal(JOB_VERSION),
    audio: renderRequestSchema.shape.source,
    preset: renderRequestSchema.shape.preset,
    fps: renderRequestSchema.shape.fps,
    size: renderRequestSchema.shape.size,
    tier: renderRequestSchema.shape.tier,
    output: renderRequestSchema.shape.output,
    neural: neuralSchema.nullable(),
  })
  .strict()

/** Where the job file of a render to `output` lives. */
export function jobFile(output: string): string {
  return `${output}.render.json`
}

/** The document for `request`, keys in the order the file shows them. */
export function jobDocument(request: RenderRequest): z.infer<typeof jobSchema> {
  return {
    version: JOB_VERSION,
    audio: request.source,
    preset: request.preset,
    fps: request.fps,
    size: request.size,
    tier: request.tier,
    output: request.output,
    neural: request.neural,
  }
}

/** Write `request`'s job file beside its output, atomically. */
export function writeJob(request: RenderRequest): string {
  const path = jobFile(request.output)
  const temporary = `${path}.tmp`
  writeFileSync(temporary, `${JSON.stringify(jobDocument(request), null, 2)}\n`, 'utf8')
  renameSync(temporary, path)
  return path
}

/**
 * The request a job file describes, or the one-line reason it is not one.
 *
 * The version is judged before anything else: a document from another version
 * may spell every other field differently, and naming the version is the
 * message that helps.
 */
export function readJob(text: string): RenderRequest {
  let json: unknown
  try {
    json = JSON.parse(text)
  } catch (error) {
    throw new Error(`the job file is not JSON: ${(error as Error).message}`)
  }
  if (typeof json !== 'object' || json === null || Array.isArray(json)) {
    throw new Error('the job file is not a render job')
  }
  const version = (json as Record<string, unknown>).version
  if (version !== JOB_VERSION) {
    throw new Error(
      `the job file is version ${JSON.stringify(version) ?? 'none'}, and this studio reads version ${JOB_VERSION}`,
    )
  }
  const parsed = jobSchema.safeParse(json)
  if (!parsed.success) {
    const first = parsed.error.issues[0]
    throw new Error(`the job file is not one this studio reads: ${first.path.join('.')} ${first.message}`)
  }
  const job = parsed.data
  return {
    source: job.audio,
    preset: job.preset,
    fps: job.fps,
    size: job.size,
    tier: job.tier,
    output: job.output,
    neural: job.neural,
  }
}
