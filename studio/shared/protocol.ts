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
export const EXPECTED_PLAYER_VERSION = '0.115.0'

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
  /**
   * The system's canonical key — `fragment_field`, the string the schema
   * document labels that system's parameter roster with. Never the scene's
   * display name: the two are the same only for the systems whose names are
   * one word, so a panel resolved from the display name finds nothing for most
   * of the roster (ADR-0184).
   */
  system: z.string(),
  /** Absolute, or `null` for a preset from the embedded set, which has none. */
  file: z.string().nullable(),
})

export const rosterSchema = z.object({
  ...base,
  ev: z.literal('roster'),
  names: z.array(z.string()),
  /**
   * The directory this reload read and the watcher polls, absolute, or `null`
   * when none resolved and the embedded set is what is running. Where a save
   * has to land to be seen; the studio never resolves it for itself.
   */
  dir: z.string().nullable(),
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
  /**
   * The preview pipe's own totals, or `null` when no pipe is open.
   *
   * The producer's count, and the only honest one: what the studio counts is
   * its own back-pressure, and a frame lost in the OS pipe never reaches it to
   * be counted. `null` rather than `0` — "no preview" and "a preview that lost
   * nothing" are different claims (ADR-0184).
   */
  preview_sent: z.number().int().nullable(),
  preview_dropped: z.number().int().nullable(),
})

/**
 * The channel orders a frame pipe can carry.
 *
 * A closed set rather than one value (ADR-0187): the headless path renders into
 * an offscreen the engine chooses and is always `rgba8`, while the windowed
 * mirror carries whatever the swapchain negotiated, which on a DX12 backend is
 * commonly `bgra8`. The player names the order it actually produces and the
 * studio reads it — a consumer that assumed one would draw the right picture in
 * the wrong colours, which reads as an authoring mistake and not a protocol one.
 */
export const PIXEL_FORMATS = ['rgba8', 'bgra8'] as const
export type PixelFormat = (typeof PIXEL_FORMATS)[number]

export function isKnownPixelFormat(format: string): format is PixelFormat {
  return (PIXEL_FORMATS as readonly string[]).includes(format)
}

export const streamSchema = z.object({
  ...base,
  ev: z.literal('stream'),
  width: z.number().int().positive(),
  height: z.number().int().positive(),
  fps: z.number().int().nonnegative(),
  /**
   * A string here and **not** a `z.enum`, deliberately.
   *
   * An enum would make a line naming an order this build does not know a
   * malformed `stream`, and a dropped `stream` is a studio that never learns
   * its geometry: a blank canvas and a count in a log. The line is accepted so
   * the refusal can be shown where the picture would be, which is the treatment
   * an unknown `hello` version already gets. `isKnownPixelFormat` is the guard.
   */
  format: z.string().min(1),
})

export const pongSchema = z.object({
  ...base,
  ev: z.literal('pong'),
  nonce: z.number().int(),
})

/**
 * Every event, by name. **The one list**: the roster the spec is diffed
 * against and the field sets that diff are both read off this, so a member
 * added to the union without a row here cannot hide.
 */
export const PLAYER_EVENT_SCHEMAS = {
  hello: helloSchema,
  preset: presetSchema,
  roster: rosterSchema,
  preset_error: presetErrorSchema,
  preset_warning: presetWarningSchema,
  health: healthSchema,
  stream: streamSchema,
  pong: pongSchema,
} as const

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
export type PresetEvent = z.infer<typeof presetSchema>
export type RosterEvent = z.infer<typeof rosterSchema>

/** Every `ev` name, read off the one list rather than typed a second time. */
export const PLAYER_EVENT_NAMES = Object.keys(PLAYER_EVENT_SCHEMAS) as PlayerEvent['ev'][]

/**
 * The fields one event carries, excluding the two that route it.
 *
 * `v` and `ev` are on every line and are not in the spec's `Fields` column, so
 * they are dropped here rather than special-cased at the comparison.
 */
export function eventFields(ev: PlayerEvent['ev']): string[] {
  return Object.keys(PLAYER_EVENT_SCHEMAS[ev].shape).filter((key) => key !== 'v' && key !== 'ev')
}

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
