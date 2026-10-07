/**
 * The Judge view: pick a set, judge it in the show window, end, write back
 * (ADR-0267).
 *
 * Main owns the session — the copy, the player restart, the ledger, the
 * write-back — and this view only asks for each step and shows the answer. The
 * verdicts are judged in the player's own window with F1 and F2, and they reach
 * this view as `marks` events, folded here by the same `verdictOf` main folds
 * with, so the rows on screen are the lines End will append.
 *
 * **The marks the app last saw at Start are not the session's.** They belong to
 * the player that was running before, on the owner's real marks file. The
 * session starts from the marks main reports for it, and a `marks` value is
 * taken only once it is a different one from what was held at Start.
 */
import { useEffect, useRef, useState } from 'react'

import {
  countVerdicts,
  latestVerdicts,
  ledgerMarkdown,
  verdictOf,
  type DeleteResult,
  type EndedSession,
  type LedgerLine,
  type MarkSets,
  type SessionDir,
  type SessionInfo,
  type SetPick,
  type SourceListing,
  type Verdict,
  type WriteBackResult,
} from '@shared/judging'

import styles from './Judge.module.css'

export interface JudgeProps {
  /** The marks the player last reported, or `undefined` before any. */
  marks: MarkSets | undefined
  /**
   * Kept mounted while hidden once opened, so an ended session's edit list
   * survives the panel being closed before Write back.
   */
  hidden?: boolean
  onClose: () => void
}

type PickKind = SetPick['kind']

const NO_MARKS: MarkSets = { favourite: [], hidden: [] }

/** The pick the form describes, or `undefined` while it describes an empty set. */
function pickOf(
  kind: PickKind,
  family: string,
  ticked: ReadonlySet<string>,
  listing: SourceListing | undefined,
): SetPick | undefined {
  if (listing === undefined) return undefined
  switch (kind) {
    case 'family':
      return listing.families.includes(family) ? { kind, prefix: family } : undefined
    case 'proposed':
      return listing.files.some((file) => file.startsWith('proposed/')) ? { kind } : undefined
    case 'list':
      return ticked.size > 0 ? { kind, files: [...ticked] } : undefined
  }
}

/** `set` with `item` added or removed. */
function toggled(set: ReadonlySet<string>, item: string, on: boolean): Set<string> {
  const next = new Set(set)
  if (on) next.add(item)
  else next.delete(item)
  return next
}

export function Judge({ marks, hidden = false, onClose }: JudgeProps): JSX.Element {
  /** `judging.sourceDir`; `undefined` until main has answered. */
  const [sourceDir, setSourceDir] = useState<string | null>()
  const [listing, setListing] = useState<SourceListing>()
  const [kind, setKind] = useState<PickKind>('family')
  const [family, setFamily] = useState('')
  const [files, setFiles] = useState<Set<string>>(new Set())

  const [session, setSession] = useState<SessionInfo | null>(null)
  const [sessionMarks, setSessionMarks] = useState<MarkSets>(NO_MARKS)
  /** The `marks` value held when the session started or the view opened on it. */
  const marksAtStart = useRef<MarkSets | undefined>(marks)

  const [ended, setEnded] = useState<EndedSession>()
  const [chosenEdits, setChosenEdits] = useState<Set<string>>(new Set())
  const [written, setWritten] = useState<WriteBackResult>()

  const [ledger, setLedger] = useState<LedgerLine[]>([])
  const [copied, setCopied] = useState(false)
  const [sessions, setSessions] = useState<SessionDir[]>([])
  const [doomed, setDoomed] = useState<Set<string>>(new Set())
  const [deleted, setDeleted] = useState<DeleteResult>()

  const [problem, setProblem] = useState<string>()
  const [busy, setBusy] = useState(false)

  const refreshRecords = (): void => {
    void window.api.judging.ledger().then(setLedger)
    void window.api.judging.listSessions().then(setSessions)
  }

  useEffect(() => {
    let live = true
    void window.api.judging.getState().then((state) => {
      if (!live) return
      setSourceDir(state.sourceDir)
      setSession(state.session)
      if (state.session !== null) setSessionMarks(state.session.marks)
    })
    refreshRecords()
    return () => {
      live = false
    }
  }, [])

  useEffect(() => {
    if (sourceDir === undefined || sourceDir === null) {
      setListing(undefined)
      return
    }
    let live = true
    void window.api.judging.listSets().then((result) => {
      if (!live) return
      if (result.ok) {
        setListing(result.value)
        setFamily((current) => current || (result.value.families[0] ?? ''))
      } else setProblem(result.reason)
    })
    return () => {
      live = false
    }
  }, [sourceDir])

  useEffect(() => {
    if (session !== null && marks !== undefined && marks !== marksAtStart.current) {
      setSessionMarks(marks)
    }
  }, [marks, session])

  const chooseSource = (): void => {
    void window.api.judging.pickSourceDir().then((result) => {
      if (result === null) return
      if (result.ok) setSourceDir(result.value)
      else setProblem(result.reason)
    })
  }

  const pick = pickOf(kind, family, files, listing)

  const start = (): void => {
    if (pick === undefined) return
    setBusy(true)
    marksAtStart.current = marks
    void window.api.judging.start(pick).then((result) => {
      setBusy(false)
      setProblem(result.ok ? undefined : result.reason)
      if (!result.ok) return
      setSession(result.value)
      setSessionMarks(result.value.marks)
      setEnded(undefined)
      setWritten(undefined)
      setDeleted(undefined)
    })
  }

  const end = (): void => {
    setBusy(true)
    void window.api.judging.end().then((result) => {
      setBusy(false)
      setProblem(result.ok ? undefined : result.reason)
      if (!result.ok) return
      setSession(null)
      setEnded(result.value)
      setChosenEdits(new Set(result.value.edited))
      refreshRecords()
    })
  }

  const writeBack = (): void => {
    setBusy(true)
    void window.api.judging.writeBack([...chosenEdits]).then((result) => {
      setBusy(false)
      setProblem(result.ok ? undefined : result.reason)
      if (result.ok) setWritten(result.value)
    })
  }

  const remove = (): void => {
    void window.api.judging.deleteSessions([...doomed]).then((result) => {
      setProblem(result.ok ? undefined : result.reason)
      if (!result.ok) return
      setDeleted(result.value)
      setDoomed(new Set())
      refreshRecords()
    })
  }

  const latest = sourceDir ? latestVerdicts(ledger, sourceDir) : []

  const copyMarkdown = (): void => {
    void navigator.clipboard.writeText(ledgerMarkdown(latest)).then(
      () => setCopied(true),
      (error: unknown) => setProblem(`the table was not copied: ${String(error)}`),
    )
  }

  const verdicts: { stem: string; name: string | null; verdict: Verdict | null }[] =
    session === null
      ? []
      : session.presets.map((preset) => ({
          ...preset,
          verdict: preset.name === null ? null : verdictOf(preset.name, sessionMarks),
        }))
  const counts = countVerdicts(verdicts.flatMap((row) => (row.verdict === null ? [] : [row.verdict])))

  return (
    <section className={styles.panel} aria-label="Judge a preset set" hidden={hidden}>
      <div className={styles.bar}>
        <h2 className={styles.heading}>Judge</h2>
        <button type="button" className={styles.button} onClick={onClose}>
          close
        </button>
      </div>

      {problem !== undefined && (
        <p className={styles.problem} role="alert">
          {problem}
        </p>
      )}

      {session === null ? (
        <fieldset className={styles.group}>
          <legend className={styles.legend}>A set</legend>
          <div className={styles.row}>
            <span className={styles.dim}>source</span>
            <code className={styles.path}>
              {sourceDir ?? (sourceDir === null ? 'no source directory set' : '…')}
            </code>
            <button type="button" className={styles.button} onClick={chooseSource}>
              {sourceDir ? 'change…' : 'choose…'}
            </button>
          </div>
          {sourceDir === null && (
            <p className={styles.note}>
              Choose the preset directory to judge from, such as a checkout&apos;s{' '}
              <code>presets/</code>. It is saved as <code>judging.sourceDir</code>.
            </p>
          )}
          {listing !== undefined && (
            <SetPicker
              listing={listing}
              kind={kind}
              onKind={setKind}
              family={family}
              onFamily={setFamily}
              files={files}
              onFile={(file, on) => setFiles((previous) => toggled(previous, file, on))}
            />
          )}
          <div className={styles.row}>
            <button
              type="button"
              className={styles.primary}
              disabled={pick === undefined || busy}
              onClick={start}
            >
              Start
            </button>
          </div>
        </fieldset>
      ) : (
        <fieldset className={styles.group}>
          <legend className={styles.legend}>Judging {session.set}</legend>
          <p className={styles.note}>
            Judge in the show window: <kbd>F1</kbd> keep, <kbd>F2</kbd> cut, leave the rest to
            tune, <kbd>F</kbd> fullscreen, the arrows to step. The Library&apos;s mark buttons work
            too.
          </p>
          <p className={styles.counts} aria-label="verdict counts">
            <span>keep {counts.keep}</span>
            <span>cut {counts.cut}</span>
            <span>tune {counts.tune}</span>
          </p>
          <ul className={styles.list} aria-label="presets in the session">
            {verdicts.map((row) => (
              <li key={row.stem} className={styles.item} data-verdict={row.verdict ?? 'none'}>
                <span className={styles.stem}>{row.stem}</span>
                <span className={styles.verdict}>{row.verdict ?? 'unreadable: no verdict'}</span>
              </li>
            ))}
          </ul>
          <div className={styles.row}>
            <button type="button" className={styles.primary} disabled={busy} onClick={end}>
              End
            </button>
          </div>
        </fieldset>
      )}

      {ended !== undefined && session === null && (
        <fieldset className={styles.group}>
          <legend className={styles.legend}>Edited in {ended.run}</legend>
          {ended.edited.length === 0 ? (
            <p className={styles.note}>No file was edited in the session.</p>
          ) : (
            <>
              <ul className={styles.list} aria-label="edited files">
                {ended.edited.map((stem) => (
                  <li key={stem} className={styles.item}>
                    <label className={styles.tick}>
                      <input
                        type="checkbox"
                        checked={chosenEdits.has(stem)}
                        onChange={(event) =>
                          setChosenEdits((previous) => toggled(previous, stem, event.target.checked))
                        }
                      />
                      <span className={styles.stem}>{stem}</span>
                    </label>
                  </li>
                ))}
              </ul>
              <div className={styles.row}>
                <button
                  type="button"
                  className={styles.button}
                  disabled={chosenEdits.size === 0 || busy}
                  onClick={writeBack}
                >
                  Write back
                </button>
              </div>
            </>
          )}
          {written !== undefined && (
            <ul className={styles.list} aria-label="write-back results">
              {written.written.map((stem) => (
                <li key={stem} className={styles.item}>
                  <span className={styles.stem}>{stem}</span>
                  <span className={styles.verdict}>written</span>
                </li>
              ))}
              {written.refused.map(({ stem, reason }) => (
                <li key={stem} className={styles.item} data-refused="true">
                  <span className={styles.stem}>{stem}</span>
                  <span className={styles.problem}>refused: {reason}</span>
                </li>
              ))}
            </ul>
          )}
        </fieldset>
      )}

      <fieldset className={styles.group}>
        <legend className={styles.legend}>Ledger</legend>
        {latest.length === 0 ? (
          <p className={styles.note}>No verdicts for this source directory yet.</p>
        ) : (
          <table className={styles.table} aria-label="latest verdicts">
            <thead>
              <tr>
                <th>preset</th>
                <th>verdict</th>
                <th>run</th>
              </tr>
            </thead>
            <tbody>
              {latest.map((line) => (
                <tr key={line.stem}>
                  <td className={styles.stem}>{line.stem}</td>
                  <td>{line.verdict}</td>
                  <td className={styles.dim}>{line.run}</td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
        <div className={styles.row}>
          <button
            type="button"
            className={styles.button}
            disabled={latest.length === 0}
            onClick={copyMarkdown}
          >
            Copy as Markdown
          </button>
          {copied && <span className={styles.dim}>copied</span>}
        </div>
      </fieldset>

      {session === null && (
        <fieldset className={styles.group}>
          <legend className={styles.legend}>Sessions</legend>
          {sessions.length === 0 ? (
            <p className={styles.note}>No session directories.</p>
          ) : (
            <ul className={styles.list} aria-label="session directories">
              {sessions.map(({ run, started }) => (
                <li key={run} className={styles.item}>
                  <label className={styles.tick}>
                    <input
                      type="checkbox"
                      checked={doomed.has(run)}
                      onChange={(event) =>
                        setDoomed((previous) => toggled(previous, run, event.target.checked))
                      }
                    />
                    <span className={styles.stem}>{run}</span>
                    {started !== null && (
                      <span className={styles.dim}>{new Date(started).toLocaleString()}</span>
                    )}
                  </label>
                </li>
              ))}
            </ul>
          )}
          <div className={styles.row}>
            <button
              type="button"
              className={styles.button}
              disabled={doomed.size === 0}
              onClick={remove}
            >
              Delete
            </button>
          </div>
          {deleted !== undefined && deleted.refused.length > 0 && (
            <ul className={styles.list} aria-label="refused deletions">
              {deleted.refused.map(({ run, reason }) => (
                <li key={run} className={styles.problem}>
                  {run}: {reason}
                </li>
              ))}
            </ul>
          )}
        </fieldset>
      )}
    </section>
  )
}

interface SetPickerProps {
  listing: SourceListing
  kind: PickKind
  onKind: (kind: PickKind) => void
  family: string
  onFamily: (family: string) => void
  files: ReadonlySet<string>
  onFile: (file: string, on: boolean) => void
}

/** The three kinds of set: a family, `proposed/`, or ticked files. */
function SetPicker({
  listing,
  kind,
  onKind,
  family,
  onFamily,
  files,
  onFile,
}: SetPickerProps): JSX.Element {
  return (
    <div className={styles.picker}>
      <label className={styles.tick}>
        <input
          type="radio"
          name="judge-set"
          checked={kind === 'family'}
          onChange={() => onKind('family')}
        />
        <span>a family</span>
      </label>
      <label className={styles.indent}>
        <span className={styles.dim}>family</span>
        <select
          value={family}
          disabled={kind !== 'family'}
          onChange={(event) => onFamily(event.target.value)}
        >
          {listing.families.map((name) => (
            <option key={name} value={name}>
              {name}
            </option>
          ))}
        </select>
      </label>
      <label className={styles.tick}>
        <input
          type="radio"
          name="judge-set"
          checked={kind === 'proposed'}
          disabled={!listing.proposed}
          onChange={() => onKind('proposed')}
        />
        <span>
          <code>proposed/</code>
          {!listing.proposed && <span className={styles.dim}> (none here)</span>}
        </span>
      </label>
      <label className={styles.tick}>
        <input
          type="radio"
          name="judge-set"
          checked={kind === 'list'}
          onChange={() => onKind('list')}
        />
        <span>a list of files</span>
      </label>
      {kind === 'list' && (
        <ul className={`${styles.list} ${styles.indent}`} aria-label="files to judge">
          {listing.files.map((file) => (
            <li key={file}>
              <label className={styles.tick}>
                <input
                  type="checkbox"
                  checked={files.has(file)}
                  onChange={(event) => onFile(file, event.target.checked)}
                />
                <span className={styles.stem}>{file}</span>
              </label>
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
