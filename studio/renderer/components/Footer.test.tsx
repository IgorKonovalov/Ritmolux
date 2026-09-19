/**
 * @vitest-environment jsdom
 *
 * A player that stopped reading its control socket says so where the operator
 * already looks for the connection (Plan 0198 Phase 5).
 *
 * The distinction this file holds is three-valued, not two: listening, stopped,
 * and a player too old to say. Folding the third into "stopped" would put a red
 * word on every healthy older player, and folding it into "listening" would
 * claim a reading nothing sent (ADR-0221).
 */
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import type { HealthEvent, HelloEvent } from '@shared/protocol'

import { Footer, controlReading } from './Footer'

afterEach(cleanup)

const HELLO: HelloEvent = {
  v: 1,
  ev: 'hello',
  version: '0.113.0',
  schema: '0965e83a83985c0e',
  control: '127.0.0.1:54321',
}

/** A `health` line as the player emits it, with the listener's readings on it. */
function health(fields: Partial<HealthEvent> = {}): HealthEvent {
  return {
    v: 1,
    ev: 'health',
    fps: 60,
    frame_ms_p50: 16,
    frame_ms_p99: 17,
    ctl_rejected: 0,
    ctl_dropped: 0,
    ctl_refused: 0,
    ctl_received: 4,
    ctl_recv_errors: 0,
    ctl_listening: true,
    preview_sent: 120,
    preview_dropped: 0,
    ...fields,
  }
}

const STATS = { delivered: 10, dropped: 0 }

describe('controlReading', () => {
  it('reports a live listener with its readings and no alarm', () => {
    const reading = controlReading(HELLO, health())
    expect(reading).toMatchObject({ address: '127.0.0.1:54321', stopped: false })
    expect(reading.title).toContain('4 datagrams received')
  })

  it('reports a listener that stopped, with what it saw before it did', () => {
    const reading = controlReading(HELLO, health({ ctl_listening: false, ctl_recv_errors: 64 }))
    expect(reading.stopped).toBe(true)
    expect(reading.title).toContain('64 receive errors')
  })

  it('claims nothing about a player that does not carry the readings', () => {
    const older = health()
    delete older.ctl_received
    delete older.ctl_recv_errors
    delete older.ctl_listening
    expect(controlReading(HELLO, older).stopped).toBe(false)
  })

  it('raises no alarm for a player that bound no socket at all', () => {
    // `ctl_listening` is false here for the same reason `control` is null —
    // there is no listener — and that is the cell's own "none", not a stop.
    const reading = controlReading({ ...HELLO, control: null }, health({ ctl_listening: false }))
    expect(reading).toMatchObject({ address: 'none', stopped: false })
  })

  it('says nothing is bound before any health line has arrived', () => {
    expect(controlReading(HELLO, undefined).stopped).toBe(false)
  })
})

describe('Footer', () => {
  it('shows a stopped listener as more than an address', () => {
    render(
      <Footer
        hello={HELLO}
        health={health({ ctl_listening: false })}
        stats={STATS}
        streamLabel="640x360 rgba8"
      />,
    )
    expect(screen.getByText('stopped listening')).toBeDefined()
  })

  it('shows only the address while the listener is reading', () => {
    render(<Footer hello={HELLO} health={health()} stats={STATS} streamLabel="640x360 rgba8" />)
    expect(screen.queryByText('stopped listening')).toBeNull()
    expect(screen.getByText('127.0.0.1:54321')).toBeDefined()
  })
})
