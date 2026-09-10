/**
 * The frame reader drops rather than queues, and the count is carried to where
 * the footer can read it (Plan 0159 Phase 1).
 */
import { describe, expect, it } from 'vitest'

import type { FrameMessage } from '@shared/frames'

import { FramePump, FrameSplitter, type FramePort } from './frames'

/** A port that records what it was given and never acknowledges on its own. */
class StalledPort implements FramePort {
  readonly posted: FrameMessage[] = []
  postMessage(message: FrameMessage): void {
    this.posted.push(message)
  }
}

function scriptedFrames(count: number, bytes: number): Buffer {
  const out = Buffer.alloc(count * bytes)
  for (let frame = 0; frame < count; frame += 1) {
    out.fill(frame + 1, frame * bytes, (frame + 1) * bytes)
  }
  return out
}

describe('FrameSplitter', () => {
  it('cuts whole frames out of an arbitrary chunking', () => {
    const seen: ArrayBuffer[] = []
    const splitter = new FrameSplitter(4, (frame) => seen.push(frame))
    // Split across every awkward boundary: mid-frame, exactly on one, and a
    // chunk carrying more than one frame.
    splitter.push(Buffer.from([1, 1]))
    splitter.push(Buffer.from([1, 1, 2, 2, 2, 2, 3]))
    splitter.push(Buffer.from([3, 3, 3, 4, 4, 4, 4]))
    expect(seen).toHaveLength(4)
    expect([...new Uint8Array(seen[0])]).toEqual([1, 1, 1, 1])
    expect([...new Uint8Array(seen[2])]).toEqual([3, 3, 3, 3])
    expect(splitter.buffered).toBe(0)
  })

  it('holds a partial frame rather than emitting it', () => {
    const seen: ArrayBuffer[] = []
    const splitter = new FrameSplitter(8, (frame) => seen.push(frame))
    splitter.push(Buffer.alloc(7))
    expect(seen).toHaveLength(0)
    expect(splitter.buffered).toBe(7)
  })

  it('hands out a buffer that owns its memory, so it can be transferred', () => {
    const seen: ArrayBuffer[] = []
    const splitter = new FrameSplitter(4, (frame) => seen.push(frame))
    splitter.push(Buffer.from([9, 9, 9, 9]))
    expect(seen[0].byteLength).toBe(4)
  })

  it('refuses a frame size that could never split a stream', () => {
    expect(() => new FrameSplitter(0, () => undefined)).toThrow()
    expect(() => new FrameSplitter(-4, () => undefined)).toThrow()
  })
})

describe('FramePump backpressure', () => {
  it('drops and counts while the renderer is stalled, and never queues', () => {
    const port = new StalledPort()
    const pump = new FramePump()
    pump.attach(port)

    const bytes = 4
    const splitter = new FrameSplitter(bytes, (frame) => pump.post(frame))
    splitter.push(scriptedFrames(3, bytes))

    // Three frames arrived; one was posted and the port has never acked, so the
    // other two are gone rather than waiting somewhere.
    expect(port.posted).toHaveLength(1)
    expect(pump.delivered).toBe(1)
    expect(pump.dropped).toBe(2)
  })

  it('carries the running count on the next frame the renderer does get', () => {
    const port = new StalledPort()
    const pump = new FramePump()
    pump.attach(port)
    const bytes = 4
    const splitter = new FrameSplitter(bytes, (frame) => pump.post(frame))

    splitter.push(scriptedFrames(3, bytes))
    pump.ack()
    splitter.push(scriptedFrames(1, bytes))

    expect(port.posted).toHaveLength(2)
    // This is the number the footer renders: it reaches the renderer inside the
    // frame message, so there is no second channel and no polling.
    expect(port.posted[1].dropped).toBe(2)
    expect(port.posted[1].delivered).toBe(2)
  })

  it('drops every frame that arrives before a port exists', () => {
    const pump = new FramePump()
    const splitter = new FrameSplitter(4, (frame) => pump.post(frame))
    splitter.push(scriptedFrames(2, 4))
    expect(pump.dropped).toBe(2)
    expect(pump.delivered).toBe(0)
  })

  it('posts the frame buffer itself, cut once out of the pipe', () => {
    // Electron's main-side port transfers ports and nothing else, so the frame
    // is carried in the message and serialized by the boundary. What this holds
    // is that nothing copies it again first.
    const posted: FrameMessage[] = []
    const port: FramePort = { postMessage: (message) => posted.push(message) }
    const pump = new FramePump()
    pump.attach(port)
    const frame = new ArrayBuffer(16)
    pump.post(frame)
    expect(posted[0].frame).toBe(frame)
  })
})
