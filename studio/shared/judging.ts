/**
 * The shapes a judging session crosses the process boundary in (ADR-0267).
 *
 * A session copies a set of presets into a directory of its own, runs the one
 * player on that copy, and folds the player's `marks` events into a verdict per
 * preset. Main owns every file; the renderer picks a set, watches the fold and
 * asks for the end. Nothing here touches Node: the renderer's project
 * type-checks it too.
 */
import { z } from 'zod'

/** What a session records for one preset. */
export const VERDICTS = ['keep', 'cut', 'tune'] as const
export type Verdict = (typeof VERDICTS)[number]

/** The two sets a `marks` event carries, as the fold reads them. */
export interface MarkSets {
  favourite: readonly string[]
  hidden: readonly string[]
}

/**
 * The verdict for the preset whose display name is `name`.
 *
 * Favourite means keep, hidden means cut, neither means tune, and favourite
 * wins when a preset carries both (ADR-0267). Marks are keyed by display name,
 * which is why a set with two files sharing a name is refused at Start.
 */
export function verdictOf(name: string, marks: MarkSets): Verdict {
  if (marks.favourite.includes(name)) return 'keep'
  if (marks.hidden.includes(name)) return 'cut'
  return 'tune'
}

/** How many presets carry each verdict. */
export type VerdictCounts = Record<Verdict, number>

export function countVerdicts(verdicts: readonly Verdict[]): VerdictCounts {
  const counts: VerdictCounts = { keep: 0, cut: 0, tune: 0 }
  for (const verdict of verdicts) counts[verdict] += 1
  return counts
}

/**
 * Which presets a session judges, drawn from `judging.sourceDir`.
 *
 * `family` is every top-level `<prefix>_*.toml`; `proposed` is every `*.toml`
 * in the `proposed/` subdirectory; `list` names files by their path relative to
 * the source directory, as the listing spelled them.
 */
export const setPickSchema = z.discriminatedUnion('kind', [
  z.object({ kind: z.literal('family'), prefix: z.string().min(1) }).strict(),
  z.object({ kind: z.literal('proposed') }).strict(),
  z.object({ kind: z.literal('list'), files: z.array(z.string().min(1)).min(1) }).strict(),
])
export type SetPick = z.infer<typeof setPickSchema>

/** The `set` field of a ledger line: `family:attractor`, `proposed`, `list:3 files`. */
export function setLabel(pick: SetPick): string {
  switch (pick.kind) {
    case 'family':
      return `family:${pick.prefix}`
    case 'proposed':
      return 'proposed'
    case 'list':
      return `list:${pick.files.length} ${pick.files.length === 1 ? 'file' : 'files'}`
  }
}

/** What the source directory offers to pick a set from. */
export interface SourceListing {
  dir: string
  /** The prefixes before the first `_` of each top-level `*.toml` stem, sorted. */
  families: string[]
  /** Whether a `proposed/` subdirectory exists. */
  proposed: boolean
  /** Every pickable file, relative to `dir`: the top level, then `proposed/`. */
  files: string[]
}

/** One preset in a running session, keyed by the copy's stem. */
export interface SessionPreset {
  stem: string
  /** The top-level `name`, or `null` when none could be read: no verdict then. */
  name: string | null
}

/** A session as the renderer sees it while it runs. */
export interface SessionInfo {
  /** The session's start, also its directory name. */
  run: string
  set: string
  /**
   * `<session>/presets/`, the directory the player runs on: every file in it
   * the studio wrote at Start, so the editor writes there in place rather than
   * forking (ADR-0189).
   */
  presetDir: string
  presets: SessionPreset[]
  /** The sets of the last `marks` event the session folded; empty before the first. */
  marks: MarkSets
}

/** What ending a session reports: the copies that differ from their sources. */
export interface EndedSession {
  run: string
  /** Stems whose copy no longer hashes to what its source did at Start. */
  edited: string[]
}

/** One write-back's outcome, file by file. */
export interface WriteBackResult {
  written: string[]
  refused: { stem: string; reason: string }[]
}

/** A session directory under the judging root. */
export interface SessionDir {
  run: string
  /** When the directory was made, ISO 8601, or `null` if it could not be read. */
  started: string | null
}

/** A clean-up's outcome, directory by directory. */
export interface DeleteResult {
  deleted: string[]
  refused: { run: string; reason: string }[]
}

/** Where the Judge view stands when it opens. */
export interface JudgingState {
  sourceDir: string | null
  /** The running session, if there is one. */
  session: SessionInfo | null
}

/**
 * One line of `<userData>/judging/ledger.jsonl`, append-only (ADR-0267).
 *
 * `source` is the source directory the set was drawn from, so a ledger shared
 * by two checkouts can be read per checkout.
 */
export const ledgerLineSchema = z.object({
  v: z.literal(1),
  run: z.string().min(1),
  set: z.string().min(1),
  source: z.string().min(1),
  stem: z.string().min(1),
  name: z.string().min(1),
  verdict: z.enum(VERDICTS),
})
export type LedgerLine = z.infer<typeof ledgerLineSchema>

/**
 * Each stem's latest verdict among `lines` drawn from `source`, sorted by stem.
 *
 * Latest by file order: the ledger is append-only, so a later line is a later
 * session.
 */
export function latestVerdicts(lines: readonly LedgerLine[], source: string): LedgerLine[] {
  const latest = new Map<string, LedgerLine>()
  for (const line of lines) if (line.source === source) latest.set(line.stem, line)
  return [...latest.values()].sort((a, b) => a.stem.localeCompare(b.stem))
}

/** `lines` as a `| preset | verdict | run |` Markdown table. */
export function ledgerMarkdown(lines: readonly LedgerLine[]): string {
  const rows = lines.map((line) => `| ${line.stem} | ${line.verdict} | ${line.run} |`)
  return ['| preset | verdict | run |', '|---|---|---|', ...rows].join('\n') + '\n'
}

/** The answer every judging call resolves to: never a rejection. */
export type JudgingResult<T> = { ok: true; value: T } | { ok: false; reason: string }
