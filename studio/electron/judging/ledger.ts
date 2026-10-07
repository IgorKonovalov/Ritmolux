/**
 * `<userData>/judging/ledger.jsonl`: one JSON object per line, one line per
 * preset per ended session (ADR-0267).
 *
 * **Append-only.** A session's lines go in as one `appendFile`, and nothing
 * here ever truncates, rewrites or reorders the file: a verdict is a record,
 * and the latest one for a stem is simply the last line naming it. State that
 * outlives the session without being a choice, so it has a file and no
 * settings key (ADR-0240).
 */
import { appendFileSync, mkdirSync, readFileSync } from 'node:fs'
import { dirname, join } from 'node:path'

import { ledgerLineSchema, type LedgerLine } from '@shared/judging'

export function ledgerFile(judgingRoot: string): string {
  return join(judgingRoot, 'ledger.jsonl')
}

/** Append `lines` in one write. Nothing is written for an empty list. */
export function appendLedger(file: string, lines: readonly LedgerLine[]): void {
  if (lines.length === 0) return
  mkdirSync(dirname(file), { recursive: true })
  appendFileSync(file, lines.map((line) => `${JSON.stringify(line)}\n`).join(''), 'utf8')
}

/**
 * Every line of the ledger that parses, in file order.
 *
 * A missing file is an empty ledger. A line that does not parse — a hand edit,
 * a line from a later format — is skipped and counted rather than failing the
 * read, because the rest of the file is still a good record.
 */
export function readLedger(file: string): { lines: LedgerLine[]; unreadable: number } {
  let text: string
  try {
    text = readFileSync(file, 'utf8')
  } catch {
    return { lines: [], unreadable: 0 }
  }
  const lines: LedgerLine[] = []
  let unreadable = 0
  for (const raw of text.split('\n')) {
    if (raw.trim() === '') continue
    try {
      const parsed = ledgerLineSchema.safeParse(JSON.parse(raw))
      if (parsed.success) lines.push(parsed.data)
      else unreadable += 1
    } catch {
      unreadable += 1
    }
  }
  return { lines, unreadable }
}
