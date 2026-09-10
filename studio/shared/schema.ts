/**
 * The engine's parameter schema, typed once for all three processes.
 *
 * `ritmolux --schema` prints this document; the engine renders it from the same
 * `ParamSpec` declarations the published parameter reference is generated from
 * (ADR-0170), so a panel built out of it cannot drift from what the engine
 * accepts. Nothing in the studio hand-lists a parameter.
 *
 * Unknown keys are **dropped, not refused**: a newer player may declare a field
 * this studio was not written against, and a panel that refused the whole
 * document over one extra key would leave the user with nothing rather than
 * with the rows it does understand.
 */
import { z } from 'zod'

/** The document version this studio reads. A document at any other is refused. */
export const SCHEMA_VERSION = 1

export const paramSpecSchema = z.object({
  name: z.string().min(1),
  default: z.number(),
  /** `[lo, hi]` — the slider's ends, and what the engine clamps to. */
  range: z.tuple([z.number(), z.number()]),
  doc: z.string(),
})
export type ParamSpec = z.infer<typeof paramSpecSchema>

/** One labelled roster: a system, or an engine stage every preset may bind. */
export const paramRosterSchema = z.object({
  name: z.string().min(1),
  params: z.array(paramSpecSchema),
})
export type ParamRoster = z.infer<typeof paramRosterSchema>

export const tableKeySchema = z.object({
  name: z.string().min(1),
  /** Rendered as text for every kind, because a table key is not a number. */
  default: z.string(),
  doc: z.string(),
  kind: z.string().min(1),
  /** Present for `enum`: the values the key accepts, and no others. */
  values: z.array(z.string()).optional(),
  /** Present for `list`: what one element is. */
  of: z.string().optional(),
  /** Present for `table` and `map`: which table one value is. */
  table: z.string().optional(),
})
export type TableKey = z.infer<typeof tableKeySchema>

export const tableSpecSchema = z.object({
  name: z.string().min(1),
  doc: z.string(),
  keys: z.array(tableKeySchema),
})
export type TableSpec = z.infer<typeof tableSpecSchema>

export const schemaDocumentSchema = z.object({
  v: z.literal(SCHEMA_VERSION),
  /** Changes when and only when the engine's declarations change. */
  hash: z.string().min(1),
  systems: z.array(paramRosterSchema),
  stages: z.array(paramRosterSchema),
  tables: z.array(tableSpecSchema),
})
export type SchemaDocument = z.infer<typeof schemaDocumentSchema>

/**
 * The rosters a preset driving `system` may bind: its own system's, then every
 * engine stage.
 *
 * The stages are in the list because the engine accepts them from any preset
 * whatever its system — the loader's known-parameter check unions them in — so
 * a panel that showed only the system's own rows would refuse to move
 * parameters the player would have accepted, and the author would have no way
 * to tell that from a bug.
 *
 * An unknown system yields the stages alone rather than nothing: the run is
 * still driving something, and the compositing parameters still apply.
 */
export function rostersFor(doc: SchemaDocument, system: string | undefined): ParamRoster[] {
  const own = doc.systems.find((roster) => roster.name === system)
  return own === undefined ? doc.stages : [own, ...doc.stages]
}
