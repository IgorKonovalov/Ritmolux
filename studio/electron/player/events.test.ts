/**
 * The event reader routes on the first byte, validates what it parses, and a
 * bad line does not cost the good ones (Plan 0159 Phase 1).
 *
 * The fixtures are lines this player actually emitted, not lines invented to
 * match the parser.
 */
import { describe, expect, it } from 'vitest'

import type { PlayerEvent } from '@shared/protocol'

import { EventReader } from './events'

interface Collected {
  events: PlayerEvent[]
  diagnostics: string[]
  malformed: { line: string; reason: string }[]
  reader: EventReader
}

function collector(): Collected {
  const events: PlayerEvent[] = []
  const diagnostics: string[] = []
  const malformed: { line: string; reason: string }[] = []
  const reader = new EventReader({
    onEvent: (event) => events.push(event),
    onDiagnostic: (line) => diagnostics.push(line),
    onMalformed: (line, reason) => malformed.push({ line, reason }),
  })
  return { events, diagnostics, malformed, reader }
}

const HELLO = '{"v":1,"ev":"hello","version":"0.113.0","schema":"0965e83a83985c0e","control":null}'
const STREAM = '{"v":1,"ev":"stream","width":640,"height":360,"fps":30,"format":"rgba8"}'
const HEALTH =
  '{"v":1,"ev":"health","fps":59.9,"frame_ms_p50":16.6,"frame_ms_p99":17.2,' +
  '"ctl_rejected":0,"ctl_dropped":0,"ctl_refused":0,"ctl_received":91,' +
  '"ctl_recv_errors":0,"ctl_listening":true,"preview_sent":1804,"preview_dropped":2}'
const HEALTH_WITHOUT_LISTENER_READINGS =
  '{"v":1,"ev":"health","fps":59.9,"frame_ms_p50":16.6,"frame_ms_p99":17.2,' +
  '"ctl_rejected":0,"ctl_dropped":0,"ctl_refused":0,"preview_sent":1804,"preview_dropped":2}'

describe('EventReader', () => {
  it('separates events from the human diagnostics by the first byte', () => {
    const c = collector()
    c.reader.push(
      `${HELLO}\n` +
        'renderer : NVIDIA GeForce RTX 3080 Laptop GPU (Dx12, DiscreteGpu)\n' +
        'publishing 640x360 as raw rgba8 frames on standard output\n' +
        `${STREAM}\n`,
    )
    expect(c.events.map((e) => e.ev)).toEqual(['hello', 'stream'])
    expect(c.diagnostics).toHaveLength(2)
    expect(c.malformed).toHaveLength(0)
  })

  it('reassembles an event split across two reads', () => {
    const c = collector()
    const cut = 30
    c.reader.push(HELLO.slice(0, cut))
    expect(c.events).toHaveLength(0)
    c.reader.push(`${HELLO.slice(cut)}\n`)
    expect(c.events).toHaveLength(1)
    expect(c.events[0]).toMatchObject({ ev: 'hello', version: '0.113.0', control: null })
  })

  it('counts a malformed line and still delivers the good ones around it', () => {
    const c = collector()
    c.reader.push(`${HELLO}\n{"v":1,"ev":"hel\n${STREAM}\n`)
    expect(c.events.map((e) => e.ev)).toEqual(['hello', 'stream'])
    expect(c.malformed).toHaveLength(1)
    expect(c.reader.malformed).toBe(1)
  })

  it('refuses a line that parses but is not an event this studio knows', () => {
    const c = collector()
    // Well-formed JSON, wrong shape: an `ev` outside the roster, and a `hello`
    // missing a field the spec's table declares.
    c.reader.push('{"v":1,"ev":"nothing-like-this"}\n')
    c.reader.push('{"v":1,"ev":"hello","version":"0.113.0"}\n')
    c.reader.push(`${STREAM}\n`)
    expect(c.malformed).toHaveLength(2)
    expect(c.events.map((e) => e.ev)).toEqual(['stream'])
  })

  it('refuses an event stream version it does not know', () => {
    const c = collector()
    c.reader.push('{"v":2,"ev":"stream","width":640,"height":360,"fps":30,"format":"rgba8"}\n')
    expect(c.events).toHaveLength(0)
    expect(c.malformed).toHaveLength(1)
  })

  it('reads a health line with the listener readings and one without them', () => {
    // The second line is what a player that predates ADR-0221 emits. Refusing
    // it would cost the studio every other figure on the line — the frame
    // times, the preview totals — over three fields it never carried.
    const c = collector()
    c.reader.push(`${HEALTH}\n${HEALTH_WITHOUT_LISTENER_READINGS}\n`)
    expect(c.malformed).toHaveLength(0)
    expect(c.events.map((e) => e.ev)).toEqual(['health', 'health'])
    expect(c.events[0]).toMatchObject({ ctl_received: 91, ctl_recv_errors: 0, ctl_listening: true })
    const older = c.events[1]
    if (older.ev !== 'health') throw new Error('the second line did not parse as health')
    expect(older.ctl_listening).toBeUndefined()
    expect(older.fps).toBe(59.9)
  })

  it('treats a trailing line with no newline as a line once the child exits', () => {
    const c = collector()
    c.reader.push(HELLO)
    expect(c.events).toHaveLength(0)
    c.reader.flush()
    expect(c.events).toHaveLength(1)
  })

  it('tolerates carriage returns, which a Windows pipe supplies', () => {
    const c = collector()
    c.reader.push(`${HELLO}\r\n`)
    expect(c.events).toHaveLength(1)
  })
})
