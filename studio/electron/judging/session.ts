/**
 * A judging session: copy a set, isolate the player on it, fold its marks,
 * append the verdicts, offer the edits back (ADR-0267).
 *
 * The session owns a directory, `<root>/sessions/<run>/`, holding `presets/`
 * (the copies the player reads), `marks.toml` (the file the player keeps its
 * marks in for the run, through `--marks`) and `sources.json` (each copy's
 * source path and the SHA-256 that source had at Start). **The session never
 * opens `marks.toml`**: the player is its only writer (ADR-0229), and every mark
 * reaches the session as a `marks` event, folded by `verdictOf`.
 *
 * Nothing is written to the ledger until the session ends, so a studio that
 * dies mid-session leaves its directory for inspection and no verdicts. No
 * directory is ever removed except by `deleteSessions`, on the user's request.
 *
 * Free of Electron so it runs under the test runner: the player restart and the
 * source directory are handed in.
 */
import { createHash } from 'node:crypto'
import {
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { dirname, join, resolve } from 'node:path'

import {
  countVerdicts,
  setLabel,
  setPickSchema,
  verdictOf,
  type DeleteResult,
  type EndedSession,
  type JudgingResult,
  type LedgerLine,
  type MarkSets,
  type SessionDir,
  type SessionInfo,
  type VerdictCounts,
  type WriteBackResult,
} from '@shared/judging'

import { writePresetAtomically } from '../preset/writer'
import { appendLedger, ledgerFile, readLedger } from './ledger'
import { readDisplayName, resolveSet, stemOf } from './sets'

/** Where the player is pointed for a session: the copies and the marks file. */
export interface PlayerLaunch {
  presetDir: string
  marks: string
}

export interface JudgingOptions {
  /** `<userData>/judging`: the ledger and every session directory live under it. */
  root: string
  /** `judging.sourceDir`, read per call so a change in the panel applies at once. */
  sourceDir: () => string | undefined
  /**
   * Restart the one player: on a session's copy, or, with `undefined`, on its
   * normal vector. A throw is reported as the call's refusal.
   */
  restartPlayer: (launch: PlayerLaunch | undefined) => void
  now?: () => Date
}

/** One copied preset, as Start recorded it. */
interface CopiedFile {
  stem: string
  source: string
  copy: string
  /** The source's SHA-256 when it was copied, or after its last write-back. */
  sha256: string
  name: string | null
}

interface Session {
  run: string
  dir: string
  set: string
  source: string
  files: CopiedFile[]
  marks: MarkSets
}

function sha256(bytes: Buffer): string {
  return createHash('sha256').update(bytes).digest('hex')
}

/** A file's SHA-256, or `undefined` when it cannot be read. */
function hashOf(path: string): string | undefined {
  try {
    return sha256(readFileSync(path))
  } catch {
    return undefined
  }
}

/** `20261007T123005Z`: ISO 8601's basic form, which every filesystem accepts as a name. */
export function runName(at: Date): string {
  return at.toISOString().replace(/[-:]/g, '').replace(/\.\d{3}Z$/, 'Z')
}

/** The extended ISO form of a run name, or `null` for a name this did not make. */
export function runStarted(run: string): string | null {
  const m = run.match(/^(\d{4})(\d{2})(\d{2})T(\d{2})(\d{2})(\d{2})Z(?:-\d+)?$/)
  return m === null ? null : `${m[1]}-${m[2]}-${m[3]}T${m[4]}:${m[5]}:${m[6]}Z`
}

function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}

export class JudgingService {
  private running: Session | undefined
  /** The last ended session, kept so its edits can be written back. */
  private ended: Session | undefined

  constructor(private readonly options: JudgingOptions) {}

  get sessionsRoot(): string {
    return join(this.options.root, 'sessions')
  }

  get ledgerPath(): string {
    return ledgerFile(this.options.root)
  }

  /** The running session, as the renderer sees it. */
  get session(): SessionInfo | null {
    const s = this.running
    if (s === undefined) return null
    return {
      run: s.run,
      set: s.set,
      presetDir: join(s.dir, 'presets'),
      presets: s.files.map((file) => ({ stem: file.stem, name: file.name })),
      marks: s.marks,
    }
  }

  /** The live keep / cut / tune counts over the readable presets. */
  get counts(): VerdictCounts | null {
    const s = this.running
    if (s === undefined) return null
    return countVerdicts(
      s.files.flatMap((file) => (file.name === null ? [] : [verdictOf(file.name, s.marks)])),
    )
  }

  start(pick: unknown): JudgingResult<SessionInfo> {
    if (this.running !== undefined) return { ok: false, reason: 'a session is already running' }
    const source = this.options.sourceDir()
    if (source === undefined) {
      return { ok: false, reason: 'no source directory is set (`judging.sourceDir`)' }
    }
    const parsed = setPickSchema.safeParse(pick)
    if (!parsed.success) return { ok: false, reason: 'that is not a set the studio can draw' }
    const files = resolveSet(source, parsed.data)
    if (!Array.isArray(files)) return { ok: false, reason: files.refused }

    const read: { stem: string; source: string; bytes: Buffer; name: string | null }[] = []
    for (const file of files) {
      let bytes: Buffer
      try {
        bytes = readFileSync(file)
      } catch (error) {
        return { ok: false, reason: `${file} could not be read: ${errorText(error)}` }
      }
      read.push({ stem: stemOf(file), source: file, bytes, name: readDisplayName(bytes.toString('utf8')) })
    }

    // Marks are keyed by display name, so two files sharing one could not be
    // told apart by the fold. Refused rather than guessed.
    const byName = new Map<string, string[]>()
    for (const file of read) {
      if (file.name !== null) byName.set(file.name, [...(byName.get(file.name) ?? []), file.stem])
    }
    const clashes = [...byName].filter(([, stems]) => stems.length > 1)
    if (clashes.length > 0) {
      const named = clashes.map(([name, stems]) => `"${name}" (${stems.join(', ')})`).join('; ')
      return { ok: false, reason: `the set has presets sharing a display name: ${named}` }
    }

    let dir: string
    let run: string
    try {
      ;({ dir, run } = this.makeSessionDir())
      const presets = join(dir, 'presets')
      mkdirSync(presets)
      for (const file of read) writeFileSync(join(presets, `${file.stem}.toml`), file.bytes)
    } catch (error) {
      return { ok: false, reason: `the session directory could not be made: ${errorText(error)}` }
    }

    const set = setLabel(parsed.data)
    const session: Session = {
      run,
      dir,
      set,
      source,
      files: read.map((file) => ({
        stem: file.stem,
        source: file.source,
        copy: join(dir, 'presets', `${file.stem}.toml`),
        sha256: sha256(file.bytes),
        name: file.name,
      })),
      marks: { favourite: [], hidden: [] },
    }
    try {
      writeFileSync(
        join(dir, 'sources.json'),
        `${JSON.stringify(
          {
            run,
            set,
            source,
            files: session.files.map((f) => ({ stem: f.stem, source: f.source, sha256: f.sha256 })),
          },
          null,
          2,
        )}\n`,
        'utf8',
      )
      this.options.restartPlayer({ presetDir: join(dir, 'presets'), marks: join(dir, 'marks.toml') })
    } catch (error) {
      return { ok: false, reason: `the session could not start: ${errorText(error)}` }
    }
    this.running = session
    this.ended = undefined
    return { ok: true, value: this.session as SessionInfo }
  }

  /** A new `<run>` directory, suffixed when a session already started this second. */
  private makeSessionDir(): { dir: string; run: string } {
    mkdirSync(this.sessionsRoot, { recursive: true })
    const base = runName((this.options.now ?? (() => new Date()))())
    for (let n = 1; ; n += 1) {
      const run = n === 1 ? base : `${base}-${n}`
      const dir = join(this.sessionsRoot, run)
      try {
        // Not recursive, so an existing directory throws rather than being reused.
        mkdirSync(dir)
        return { dir, run }
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error
      }
    }
  }

  /** Fold one `marks` event. Outside a session it is not the session's. */
  marks(event: MarkSets): void {
    if (this.running === undefined) return
    this.running.marks = { favourite: [...event.favourite], hidden: [...event.hidden] }
  }

  /**
   * Append the verdicts, report the edited copies, and put the player back.
   *
   * The ledger write comes first: if it fails the session is still running, so
   * the verdicts are not lost and End can be pressed again.
   */
  end(): JudgingResult<EndedSession> {
    const s = this.running
    if (s === undefined) return { ok: false, reason: 'no session is running' }
    const lines: LedgerLine[] = s.files.flatMap((file) =>
      file.name === null
        ? []
        : [
            {
              v: 1 as const,
              run: s.run,
              set: s.set,
              source: s.source,
              stem: file.stem,
              name: file.name,
              verdict: verdictOf(file.name, s.marks),
            },
          ],
    )
    try {
      appendLedger(this.ledgerPath, lines)
    } catch (error) {
      return { ok: false, reason: `the ledger could not be written: ${errorText(error)}` }
    }
    const edited = s.files.filter((file) => {
      const now = hashOf(file.copy)
      return now !== undefined && now !== file.sha256
    })
    this.running = undefined
    this.ended = s
    try {
      this.options.restartPlayer(undefined)
    } catch (error) {
      return { ok: false, reason: `the session ended but the player did not restart: ${errorText(error)}` }
    }
    return { ok: true, value: { run: s.run, edited: edited.map((file) => file.stem) } }
  }

  /**
   * Write the chosen copies of the last ended session over their sources.
   *
   * Each source is hashed again first, and one that no longer matches what was
   * copied — edited, replaced or removed during the session — is refused by
   * name rather than overwritten: the write-back never merges.
   */
  async writeBack(stems: unknown): Promise<JudgingResult<WriteBackResult>> {
    const s = this.ended
    if (s === undefined) return { ok: false, reason: 'no ended session has edits to write back' }
    if (!Array.isArray(stems) || !stems.every((stem) => typeof stem === 'string')) {
      return { ok: false, reason: 'a write-back names the presets by stem' }
    }
    const result: WriteBackResult = { written: [], refused: [] }
    for (const stem of stems as string[]) {
      const file = s.files.find((f) => f.stem === stem)
      if (file === undefined) {
        result.refused.push({ stem, reason: 'not a preset of the session' })
        continue
      }
      if (hashOf(file.source) !== file.sha256) {
        result.refused.push({ stem, reason: `${file.source} changed since it was copied` })
        continue
      }
      try {
        const text = readFileSync(file.copy, 'utf8')
        await writePresetAtomically(file.source, text)
        file.sha256 = sha256(Buffer.from(text, 'utf8'))
        result.written.push(stem)
      } catch (error) {
        result.refused.push({ stem, reason: errorText(error) })
      }
    }
    return { ok: true, value: result }
  }

  /** Every ledger line that parses, in file order. */
  ledger(): LedgerLine[] {
    return readLedger(this.ledgerPath).lines
  }

  /** The session directories on disk, newest first. */
  listSessions(): SessionDir[] {
    let names: string[]
    try {
      names = readdirSync(this.sessionsRoot, { withFileTypes: true })
        .filter((entry) => entry.isDirectory())
        .map((entry) => entry.name)
    } catch {
      return []
    }
    return names.sort().reverse().map((run) => ({ run, started: runStarted(run) }))
  }

  /**
   * Remove the named session directories, refusing each that is not one.
   *
   * A name is resolved under the sessions root and must land directly in it,
   * so `..` or an absolute path cannot aim the removal anywhere else; the
   * running session's directory is the player's preset directory and is
   * refused while it runs.
   */
  deleteSessions(runs: unknown): JudgingResult<DeleteResult> {
    if (!Array.isArray(runs) || !runs.every((run) => typeof run === 'string')) {
      return { ok: false, reason: 'a clean-up names the sessions by directory' }
    }
    const root = resolve(this.sessionsRoot)
    const result: DeleteResult = { deleted: [], refused: [] }
    for (const run of runs as string[]) {
      const full = resolve(root, run)
      if (run === '' || dirname(full) !== root) {
        result.refused.push({ run, reason: 'not a session directory under the judging root' })
        continue
      }
      if (this.running !== undefined && resolve(this.running.dir) === full) {
        result.refused.push({ run, reason: 'the running session' })
        continue
      }
      try {
        if (!statSync(full).isDirectory()) throw new Error('not a directory')
      } catch {
        result.refused.push({ run, reason: 'no such session' })
        continue
      }
      try {
        rmSync(full, { recursive: true, force: true })
        result.deleted.push(run)
        if (this.ended !== undefined && resolve(this.ended.dir) === full) this.ended = undefined
      } catch (error) {
        result.refused.push({ run, reason: errorText(error) })
      }
    }
    return { ok: true, value: result }
  }
}
