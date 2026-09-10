/**
 * `protocol.ts` and the spec's tables are held equal, both ways
 * (Plan 0159 Phase 2).
 *
 * `docs/specs/0003-studio-control-protocol.md` is the source and this file is
 * the check, in the way `presets/README.md` is held to `ParamSpec` (ADR-0170).
 * The diff runs in **both** directions on purpose: a row in the spec with no
 * union member means the studio cannot say something the player accepts, and a
 * union member with no row means the studio invented a message. Either is a
 * fork, and a widening on one side fails here until the other moves.
 */
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import {
  CTL_ADDRESSES,
  PLAYER_EVENT_NAMES,
  TRANSPORT_VERBS,
  eventFields,
  playerEventSchema,
} from './protocol'

const SPEC = join(__dirname, '..', '..', 'docs', 'specs', '0003-studio-control-protocol.md')

/** A pipe escaped for markdown, as the two characters it is. */
const ESCAPED_PIPE = String.fromCharCode(92) + '|'
/** Stands in for one while the row is split, so the split does not tear it. */
const PLACEHOLDER = String.fromCharCode(0)

/**
 * The cells of a markdown table row.
 *
 * Splits on unescaped pipes only: the transport row writes its verbs as an
 * escaped list, and a naive split would tear that one cell into four. Done with
 * literal splits rather than a regex because the pattern for an escaped pipe is
 * a backslash thicket, and one backslash lost in transit turns it into a
 * pattern that matches everywhere.
 */
function cells(row: string): string[] {
  return row
    .split(ESCAPED_PIPE)
    .join(PLACEHOLDER)
    .split('|')
    .slice(1, -1)
    .map((cell) => cell.trim().split(PLACEHOLDER).join('|'))
}

/** Every row of the table under `heading`, as cell arrays. */
function tableUnder(heading: string): string[][] {
  const text = readFileSync(SPEC, 'utf8')
  const start = text.indexOf(heading)
  if (start === -1) throw new Error(`the spec has no ${heading} section`)
  const rows: string[][] = []
  let seenSeparator = false
  for (const line of text.slice(start).split('\n').slice(1)) {
    if (!line.trimStart().startsWith('|')) {
      if (rows.length > 0) break
      continue
    }
    if (/^\|[\s:-]+\|/.test(line)) {
      seenSeparator = true
      continue
    }
    if (seenSeparator) rows.push(cells(line))
  }
  if (rows.length === 0) throw new Error(`no table found under ${heading}`)
  return rows
}

/** The text inside the first pair of backticks in a cell. */
function code(cell: string): string {
  const match = cell.match(/`([^`]+)`/)
  if (match === null) throw new Error(`no code span in cell: ${cell}`)
  return match[1]
}

/** Every backticked name in a cell, in order — the `Fields` column's shape. */
function codes(cell: string): string[] {
  return [...cell.matchAll(/`([^`]+)`/g)].map((match) => match[1])
}

describe('the vocabulary table and the CtlAction union', () => {
  const rows = tableUnder('## The vocabulary')
  const specAddresses = rows.map((row) => code(row[0]))
  const ours: string[] = Object.values(CTL_ADDRESSES)

  it('reads a table that still looks like the one this test was written for', () => {
    expect(specAddresses.length).toBeGreaterThan(0)
    for (const address of specAddresses) expect(address.startsWith('/rlx/v1/ctl/')).toBe(true)
  })

  it('has a union member for every address the spec declares', () => {
    expect([...specAddresses].sort()).toEqual([...ours].sort())
  })

  it('declares no address the spec does not', () => {
    for (const address of ours) expect(specAddresses).toContain(address)
  })

  it('names one address per union member, with no two sharing one', () => {
    expect(new Set(ours).size).toBe(ours.length)
  })

  it('carries exactly the transport verbs the spec names', () => {
    const row = rows.find((cs) => code(cs[0]) === '/rlx/v1/ctl/transport')
    if (row === undefined) throw new Error('the spec declares no transport row')
    // The cell reads `s next|prev|auto|hold`; the verbs are what follows `s `.
    const verbs = code(row[1]).replace(/^s\s+/, '').split('|')
    expect(verbs).toEqual([...TRANSPORT_VERBS])
  })
})

describe('the event roster table and the PlayerEvent union', () => {
  const specEvents = tableUnder('## The event roster').map((row) => code(row[0]))

  it('reads a table that still looks like the one this test was written for', () => {
    expect(specEvents).toContain('hello')
    expect(specEvents.length).toBeGreaterThan(4)
  })

  it('has a union member for every event the spec declares', () => {
    expect([...specEvents].sort()).toEqual([...PLAYER_EVENT_NAMES].sort())
  })

  it('declares no event the spec does not', () => {
    for (const name of PLAYER_EVENT_NAMES) expect(specEvents).toContain(name)
  })

  it('carries a union member per name, so a name cannot be listed and unparsed', () => {
    expect(playerEventSchema.options.length).toBe(PLAYER_EVENT_NAMES.length)
  })
})

/**
 * The **fields**, not only the names.
 *
 * The name diff above is what Phase 2 asked for, and it is blind to a field:
 * `preset` gained `system` and `file`, `roster` gained `dir` and `health`
 * gained two counters, and every one of them could have been added on one side
 * alone without a red test. A field is exactly as much of the contract as an
 * event is — a parent reads it and branches on it — so it is diffed the same
 * way, and in both directions (ADR-0184).
 */
describe('the event roster table and the fields each union member carries', () => {
  const rows = tableUnder('## The event roster')

  it('reads a Fields column that still looks like the one this test was written for', () => {
    const hello = rows.find((cs) => code(cs[0]) === 'hello')
    if (hello === undefined) throw new Error('the spec declares no hello row')
    expect(codes(hello[1])).toEqual(['version', 'schema', 'control'])
  })

  it('agrees with the spec on every field of every event, both ways', () => {
    for (const row of rows) {
      const ev = code(row[0]) as (typeof PLAYER_EVENT_NAMES)[number]
      const spec = codes(row[1])
      const ours = eventFields(ev)
      // Order is part of it: the spec's column reads in the order the writer
      // emits, and a reader following the table gets the line's own shape.
      expect(ours, `the fields of ${ev}`).toEqual(spec)
    }
  })
})
