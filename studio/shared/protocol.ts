/**
 * The control protocol, typed once for all three processes.
 *
 * `docs/specs/0003-studio-control-protocol.md` is the source; this file is held
 * to its two tables by a test rather than kept in step by hand. Every event the
 * main process parses off the player's standard error goes through the schema
 * below before anything downstream sees it — the boundary where an unknown
 * shape is refused, so the renderer can trust its types (ADR-0178).
 *
 * A field the player emits as JSON `null` is modelled as `null`, not as an
 * absent key: the writer emits the key either way, and a consumer that treats
 * "missing" and "null" alike would accept a line the spec does not allow.
 */
import { z } from 'zod'

/** The event stream's own version. A line carrying any other `v` is refused. */
export const EVENT_VERSION = 1

/**
 * The player build this studio was written against.
 *
 * ADR-0178: the studio refuses a player whose `hello` reports a version it does
 * not know, because a preview that silently drives a different engine is worse
 * than a visible refusal. The two ship in one zip, so an exact match is the
 * rule and a mismatch means the bundle was assembled wrong or a path in
 * settings points at a stale build.
 */
export const EXPECTED_PLAYER_VERSION = '0.113.0'

export function isKnownPlayerVersion(version: string): boolean {
  return version === EXPECTED_PLAYER_VERSION
}

const base = { v: z.literal(EVENT_VERSION) }

export const helloSchema = z.object({
  ...base,
  ev: z.literal('hello'),
  version: z.string(),
  schema: z.string(),
  /** `host:port` actually bound, or `null` when no listener was opened. */
  control: z.string().nullable(),
})

export const presetSchema = z.object({
  ...base,
  ev: z.literal('preset'),
  name: z.string(),
  index: z.number().int(),
})

export const rosterSchema = z.object({
  ...base,
  ev: z.literal('roster'),
  names: z.array(z.string()),
})

export const presetErrorSchema = z.object({
  ...base,
  ev: z.literal('preset_error'),
  file: z.string(),
  message: z.string(),
  line: z.number().int().nullable(),
  col: z.number().int().nullable(),
  /** Set instead of a span when the failure is an expression, which has none. */
  param: z.string().nullable(),
})

export const presetWarningSchema = z.object({
  ...base,
  ev: z.literal('preset_warning'),
  file: z.string(),
  message: z.string(),
})

export const healthSchema = z.object({
  ...base,
  ev: z.literal('health'),
  fps: z.number(),
  frame_ms_p50: z.number(),
  frame_ms_p99: z.number(),
  ctl_rejected: z.number().int(),
  ctl_dropped: z.number().int(),
  ctl_refused: z.number().int(),
})

export const streamSchema = z.object({
  ...base,
  ev: z.literal('stream'),
  width: z.number().int().positive(),
  height: z.number().int().positive(),
  fps: z.number().int().nonnegative(),
  format: z.literal('rgba8'),
})

export const pongSchema = z.object({
  ...base,
  ev: z.literal('pong'),
  nonce: z.number().int(),
})

export const playerEventSchema = z.discriminatedUnion('ev', [
  helloSchema,
  presetSchema,
  rosterSchema,
  presetErrorSchema,
  presetWarningSchema,
  healthSchema,
  streamSchema,
  pongSchema,
])

export type PlayerEvent = z.infer<typeof playerEventSchema>
export type HelloEvent = z.infer<typeof helloSchema>
export type StreamEvent = z.infer<typeof streamSchema>
export type HealthEvent = z.infer<typeof healthSchema>
export type PresetErrorEvent = z.infer<typeof presetErrorSchema>

/** Every `ev` name the union carries, for the test that diffs it to the spec. */
export const PLAYER_EVENT_NAMES = [
  'hello',
  'preset',
  'roster',
  'preset_error',
  'preset_warning',
  'health',
  'stream',
  'pong',
] as const

/**
 * Bytes one frame of the geometry a `stream` event declared occupies.
 *
 * The sink writes tight rows with no padding and no framing of any kind, so the
 * only thing that separates one frame from the next on the pipe is this count.
 */
export function frameBytes(stream: Pick<StreamEvent, 'width' | 'height'>): number {
  return stream.width * stream.height * 4
}

// ---------------------------------------------------------------------------
// The vocabulary: what the studio may send
// ---------------------------------------------------------------------------

/** The prefix every address carries, version included. */
export const ADDRESS_PREFIX = '/rlx/v1'

/** The transport verbs the spec's table names, and no others. */
export const TRANSPORT_VERBS = ['next', 'prev', 'auto', 'hold'] as const
export type TransportVerb = (typeof TRANSPORT_VERBS)[number]

export const ctlActionSchema = z.discriminatedUnion('kind', [
  z.object({ kind: z.literal('param'), name: z.string().min(1), value: z.number().finite() }),
  z.object({ kind: z.literal('param_clear'), name: z.string().min(1) }),
  z.object({ kind: z.literal('params_clear') }),
  z.object({ kind: z.literal('preset'), name: z.string().min(1) }),
  z.object({ kind: z.literal('transport'), verb: z.enum(TRANSPORT_VERBS) }),
  z.object({ kind: z.literal('ping'), nonce: z.number().int() }),
])

export type CtlAction = z.infer<typeof ctlActionSchema>

/**
 * The address each action resolves to.
 *
 * One entry per row of the spec's vocabulary table, and the test that diffs the
 * two ways round is what keeps it that way — an address added here without a
 * row, or a row added without an address, fails before it can ship.
 */
export const CTL_ADDRESSES = {
  param: `${ADDRESS_PREFIX}/ctl/param`,
  param_clear: `${ADDRESS_PREFIX}/ctl/param/clear`,
  params_clear: `${ADDRESS_PREFIX}/ctl/params/clear`,
  preset: `${ADDRESS_PREFIX}/ctl/preset`,
  transport: `${ADDRESS_PREFIX}/ctl/transport`,
  ping: `${ADDRESS_PREFIX}/ctl/ping`,
} as const satisfies Record<CtlAction['kind'], string>

export function ctlAddress(action: CtlAction): string {
  return CTL_ADDRESSES[action.kind]
}
