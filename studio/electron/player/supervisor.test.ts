/**
 * The supervisor reads both pipes, refuses a player it does not know, and does
 * not lose output that arrived before the geometry did (Plan 0159 Phase 1).
 */
import { EventEmitter } from 'node:events'
import { PassThrough } from 'node:stream'

import { describe, expect, it, vi } from 'vitest'

import type { FrameMessage } from '@shared/frames'
import { EXPECTED_PLAYER_VERSION, PIXEL_FORMATS, type PlayerEvent } from '@shared/protocol'

import { PlayerSupervisor, DEFAULT_PLAYER_ARGS, type SpawnFn } from './supervisor'
import type { FramePort } from './frames'

/** A child whose two pipes the test writes to by hand. */
class FakeChild extends EventEmitter {
  readonly stdout = new PassThrough()
  readonly stderr = new PassThrough()
  readonly kill = vi.fn()
}

function harness(): {
  child: FakeChild
  supervisor: PlayerSupervisor
  events: PlayerEvent[]
  refusals: string[]
  posted: FrameMessage[]
} {
  const child = new FakeChild()
  const events: PlayerEvent[] = []
  const refusals: string[] = []
  const posted: FrameMessage[] = []
  const port: FramePort = { postMessage: (message) => posted.push(message) }
  const spawn = (() => child) as unknown as SpawnFn
  const supervisor = new PlayerSupervisor({
    playerPath: '/bin/ritmolux',
    spawn,
    onEvent: (event) => events.push(event),
    onDiagnostic: () => undefined,
    onMalformed: () => undefined,
    onRefused: (reason) => refusals.push(reason),
    onExit: () => undefined,
  })
  supervisor.start()
  supervisor.pump.attach(port)
  return { child, supervisor, events, refusals, posted }
}

const hello = (version: string): string =>
  `{"v":1,"ev":"hello","version":"${version}","schema":"0965e83a83985c0e","control":null}\n`
const STREAM = '{"v":1,"ev":"stream","width":2,"height":1,"fps":30,"format":"rgba8"}\n'

describe('PlayerSupervisor', () => {
  it('asks one windowed player to mirror the show, report, and listen', () => {
    // `--preview stdout` rather than `--stream --sink stdout`: the studio drives
    // the show itself and paints a copy of its frames, so there is no second
    // player and no second capture (ADR-0181). Port 0 asks for an ephemeral
    // listener; `hello.control` is the only place the answer comes from, which
    // is why nothing here predicts it.
    expect(DEFAULT_PLAYER_ARGS).toEqual([
      '--preview',
      'stdout',
      '--events',
      '--control',
      '127.0.0.1:0',
    ])
  })

  it('names no geometry and no channel order, because the player reports both', () => {
    // A size or a rate here would be a guess at a windowed run's swapchain and
    // surface, and the frame splitter would then cut the pipe by the guess
    // rather than by what arrived. The `stream` event is the only source, and
    // since ADR-0187 that is true of the channel order as well.
    for (const flag of ['--size', '--fps']) {
      expect(DEFAULT_PLAYER_ARGS).not.toContain(flag)
    }
    for (const format of PIXEL_FORMATS) {
      expect(DEFAULT_PLAYER_ARGS).not.toContain(format)
    }
  })

  it('reports the control address the player actually bound', () => {
    const h = harness()
    h.child.stderr.write(
      '{"v":1,"ev":"hello","version":"' +
        EXPECTED_PLAYER_VERSION +
        '","schema":"a","control":"127.0.0.1:54321"}\n',
    )
    expect(h.supervisor.control).toBe('127.0.0.1:54321')
  })

  it('reports no control address when the player opened no listener', () => {
    const h = harness()
    h.child.stderr.write(hello(EXPECTED_PLAYER_VERSION))
    expect(h.supervisor.control).toBeNull()
  })

  it('refuses and stops a player whose version it does not drive', () => {
    const h = harness()
    h.child.stderr.write(hello('0.1.0'))
    expect(h.refusals).toHaveLength(1)
    expect(h.refusals[0]).toContain('0.1.0')
    // Stopped, not merely reported: a refused player keeps a GPU and a capture.
    expect(h.child.kill).toHaveBeenCalled()
    // The `hello` still reaches the renderer, which is what puts the refusal on
    // screen instead of leaving a blank preview.
    expect(h.events.map((e) => e.ev)).toEqual(['hello'])
  })

  it('splits frames once the geometry has been declared', () => {
    const h = harness()
    h.child.stderr.write(hello(EXPECTED_PLAYER_VERSION))
    h.child.stderr.write(STREAM)
    h.child.stdout.write(Buffer.alloc(8, 7))
    expect(h.posted).toHaveLength(1)
    expect(h.posted[0].frame.byteLength).toBe(8)
  })

  it('reads no frames from a pipe whose channel order it cannot name', () => {
    const h = harness()
    h.child.stderr.write(hello(EXPECTED_PLAYER_VERSION))
    h.child.stderr.write(STREAM.replace('rgba8', 'rgbx9'))

    expect(h.refusals).toHaveLength(1)
    expect(h.refusals[0]).toContain('rgbx9')
    // The event still reaches the renderer, which is what puts the refusal
    // where the picture would be rather than leaving a blank canvas.
    expect(h.events.map((e) => e.ev)).toEqual(['hello', 'stream'])
    // The player keeps drawing: it is a valid show, and its frames are read and
    // dropped rather than painted on a guess or held until memory runs out.
    expect(h.child.kill).not.toHaveBeenCalled()
    h.child.stdout.write(Buffer.alloc(8, 7))
    expect(h.posted).toHaveLength(0)
  })

  it('holds output that arrives before the stream event rather than losing it', () => {
    const h = harness()
    h.child.stderr.write(hello(EXPECTED_PLAYER_VERSION))
    // The two pipes are independent, so this ordering is possible even though
    // the player writes the event first.
    h.child.stdout.write(Buffer.alloc(8, 3))
    expect(h.posted).toHaveLength(0)
    h.child.stderr.write(STREAM)
    expect(h.posted).toHaveLength(1)
    expect(new Uint8Array(h.posted[0].frame)[0]).toBe(3)
  })
})
