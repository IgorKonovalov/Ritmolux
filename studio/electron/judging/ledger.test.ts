/**
 * The ledger appends and reads back, skips what does not parse, and the latest
 * verdict per stem and its Markdown table are read off it (Plan 0254 Phase 2).
 */
import { appendFileSync, mkdtempSync, readFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import { latestVerdicts, ledgerMarkdown, type LedgerLine } from '@shared/judging'

import { appendLedger, readLedger } from './ledger'

const line = (stem: string, verdict: LedgerLine['verdict'], run: string, source = '/p'): LedgerLine => ({
  v: 1,
  run,
  set: 'family:a',
  source,
  stem,
  name: stem.toUpperCase(),
  verdict,
})

describe('the ledger', () => {
  it('appends after what is there and reads every line back', () => {
    const file = join(mkdtempSync(join(tmpdir(), 'rlx-ledger-')), 'judging', 'ledger.jsonl')
    appendLedger(file, [line('a_x', 'keep', 'r1')])
    const before = readFileSync(file, 'utf8')
    appendLedger(file, [line('a_x', 'cut', 'r2'), line('a_y', 'tune', 'r2')])
    expect(readFileSync(file, 'utf8').startsWith(before)).toBe(true)
    expect(readLedger(file)).toEqual({
      lines: [line('a_x', 'keep', 'r1'), line('a_x', 'cut', 'r2'), line('a_y', 'tune', 'r2')],
      unreadable: 0,
    })
  })

  it('skips and counts a line it cannot read', () => {
    const file = join(mkdtempSync(join(tmpdir(), 'rlx-ledger-')), 'ledger.jsonl')
    appendLedger(file, [line('a_x', 'keep', 'r1')])
    appendFileSync(file, 'not json\n{"v":2}\n')
    expect(readLedger(file).lines).toHaveLength(1)
    expect(readLedger(file).unreadable).toBe(2)
  })

  it('reads a missing ledger as empty', () => {
    expect(readLedger(join(tmpdir(), 'rlx-ledger-nothing-here.jsonl'))).toEqual({
      lines: [],
      unreadable: 0,
    })
  })

  it('takes the latest verdict per stem for one source and renders it as a table', () => {
    const lines = [
      line('a_y', 'tune', 'r1'),
      line('a_x', 'tune', 'r1'),
      line('a_x', 'keep', 'r2'),
      line('a_x', 'cut', 'r3', '/elsewhere'),
    ]
    const latest = latestVerdicts(lines, '/p')
    expect(latest.map((l) => [l.stem, l.verdict, l.run])).toEqual([
      ['a_x', 'keep', 'r2'],
      ['a_y', 'tune', 'r1'],
    ])
    expect(ledgerMarkdown(latest)).toBe(
      '| preset | verdict | run |\n|---|---|---|\n| a_x | keep | r2 |\n| a_y | tune | r1 |\n',
    )
  })
})
