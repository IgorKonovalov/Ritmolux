/**
 * The schema document is validated at the boundary, and read at most once
 * (Plan 0159 Phase 6).
 *
 * Every parameter row in the studio is generated from this document, so a
 * document from a build the studio does not understand has to be refused **with
 * a reason** rather than half-parsed into a panel with missing rows.
 */
import { describe, expect, it, vi } from 'vitest'

import { parseSchemaDocument, SchemaCache, SchemaError } from './schema'

const MINIMAL = JSON.stringify({
  v: 1,
  hash: '0123456789abcdef',
  systems: [
    { name: 'spectrum', params: [{ name: 'warp', default: 0.4, range: [0, 1.5], doc: 'A fold.' }] },
  ],
  stages: [{ name: 'bloom', params: [] }],
  tables: [{ name: 'preset', doc: 'The document.', keys: [] }],
  grammar: { variables: ['bass'], functions: ['sin'], constants: ['pi'] },
})

describe('parsing a document', () => {
  it('reads the rosters and the tables the engine declared', () => {
    const doc = parseSchemaDocument(MINIMAL)
    expect(doc.systems[0].name).toBe('spectrum')
    expect(doc.systems[0].params[0]).toEqual({
      name: 'warp',
      default: 0.4,
      range: [0, 1.5],
      doc: 'A fold.',
      // A document from before the engine declared kinds still reads, and its
      // parameters read as continuous — the answer every parameter had then.
      kind: 'modal',
    })
    expect(doc.grammar.functions).toEqual(['sin'])
  })

  it('keeps the rows it understands when a newer engine adds a field it does not', () => {
    // A player ahead of this studio declares more per parameter. Refusing the
    // whole document over one extra key would leave the panel empty rather than
    // merely missing that key's affordance.
    const ahead = JSON.parse(MINIMAL) as Record<string, unknown>
    const systems = ahead.systems as { params: Record<string, unknown>[] }[]
    systems[0].params[0].integer = true
    expect(parseSchemaDocument(JSON.stringify(ahead)).systems[0].params[0].name).toBe('warp')
  })

  it('refuses a document at a version it does not read, and says which field', () => {
    const older = JSON.stringify({ ...JSON.parse(MINIMAL), v: 2 })
    expect(() => parseSchemaDocument(older)).toThrow(SchemaError)
    expect(() => parseSchemaDocument(older)).toThrow(/v/)
  })

  it('refuses something that is not JSON at all, with what the parser said', () => {
    expect(() => parseSchemaDocument('loaded 41 preset(s) from ...')).toThrow(/not JSON/)
  })

  it('reads a null range, which is what an unbounded parameter carries', () => {
    // Thirty-one of the engine's parameters declare no bounds — the pan
    // offsets and their siblings. Refusing the document over one would leave
    // the panel empty for every system that has one.
    const unbounded = JSON.parse(MINIMAL) as Record<string, unknown>
    const systems = unbounded.systems as { params: Record<string, unknown>[] }[]
    systems[0].params[0].range = null
    expect(parseSchemaDocument(JSON.stringify(unbounded)).systems[0].params[0].range).toBeNull()
  })

  it('refuses a roster whose parameter has no range key at all', () => {
    const broken = JSON.parse(MINIMAL) as Record<string, unknown>
    const systems = broken.systems as { params: Record<string, unknown>[] }[]
    delete systems[0].params[0].range
    // Absent is not the same as declared-unbounded, and a document missing the
    // key is one this studio was not written against.
    expect(() => parseSchemaDocument(JSON.stringify(broken))).toThrow(SchemaError)
  })
})

describe('the cache', () => {
  it('runs the player once however many callers ask', async () => {
    const run = vi.fn().mockResolvedValue(MINIMAL)
    const cache = new SchemaCache('/somewhere/ritmolux', run)
    const [a, b] = await Promise.all([cache.get(), cache.get()])
    expect(run).toHaveBeenCalledTimes(1)
    expect(a).toBe(b)
  })

  it('does not spawn a process per caller when there is no player', async () => {
    const run = vi.fn()
    const cache = new SchemaCache(undefined, run)
    await expect(cache.get()).rejects.toThrow(/no player/)
    expect(run).not.toHaveBeenCalled()
  })

  it('keeps the failure rather than retrying a player that cannot answer', async () => {
    const run = vi.fn().mockRejectedValue(new SchemaError('the binary is not one'))
    const cache = new SchemaCache('/somewhere/ritmolux', run)
    await expect(cache.get()).rejects.toThrow()
    await expect(cache.get()).rejects.toThrow()
    expect(run).toHaveBeenCalledTimes(1)
  })
})
