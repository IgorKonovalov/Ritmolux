/**
 * Standard output to frames, and frames to the renderer's port.
 *
 * Two pieces with one rule between them: **the main process never blocks on the
 * child's pipe** (ADR-0178). [`FrameSplitter`] turns an arbitrary chunk stream
 * into whole frames; [`FramePump`] posts them and drops the ones the renderer
 * has not kept up with. Backpressure here is a dropped frame and a count, never
 * a queue that grows or a read that stalls — a stalled read fills the pipe and
 * the player blocks writing to it, which would make a slow preview into a
 * stuttering show.
 */
import type { FrameMessage } from '@shared/frames'

/**
 * Splits a byte stream into fixed-size frames.
 *
 * Chunks are held in a list and copied out once, rather than concatenated into
 * one growing buffer per chunk — the naive form is quadratic, and at 640x360
 * this sees about 27 MB a second.
 */
export class FrameSplitter {
  private readonly pending: Buffer[] = []
  private pendingBytes = 0

  constructor(
    private readonly frameBytes: number,
    private readonly onFrame: (frame: ArrayBuffer) => void,
  ) {
    if (!Number.isInteger(frameBytes) || frameBytes <= 0) {
      throw new Error(`frameBytes must be a positive integer, got ${frameBytes}`)
    }
  }

  push(chunk: Buffer): void {
    this.pending.push(chunk)
    this.pendingBytes += chunk.length
    while (this.pendingBytes >= this.frameBytes) {
      this.onFrame(this.take(this.frameBytes))
    }
  }

  /** Bytes held back because they are less than a whole frame. */
  get buffered(): number {
    return this.pendingBytes
  }

  /**
   * Copy `n` bytes out of the pending list into an `ArrayBuffer` that owns its
   * memory. It has to own it: the buffer is transferred to the renderer, and a
   * pooled `Buffer`'s backing store is shared with unrelated allocations.
   */
  private take(n: number): ArrayBuffer {
    const out = new ArrayBuffer(n)
    const view = Buffer.from(out)
    let filled = 0
    while (filled < n) {
      const head = this.pending[0]
      const need = n - filled
      if (head.length <= need) {
        head.copy(view, filled)
        filled += head.length
        this.pending.shift()
      } else {
        head.copy(view, filled, 0, need)
        filled += need
        this.pending[0] = head.subarray(need)
      }
    }
    this.pendingBytes -= n
    return out
  }
}

/**
 * The port side of the pump, narrowed to what it uses of a `MessagePortMain`.
 *
 * No transfer list, because none is possible in this direction: Electron's
 * `MessagePortMain.postMessage(message, transfer)` accepts **only
 * `MessagePortMain` objects** in `transfer`, and main and the renderer are
 * separate OS processes, so a frame's bytes are serialized and copied whatever
 * is asked for. What is avoided instead is a second copy on each side: the
 * buffer is cut once out of the pipe's chunks and viewed in place by
 * `ImageData`.
 */
export interface FramePort {
  postMessage(message: FrameMessage): void
}

/**
 * Posts frames to the renderer, one in flight at a time.
 *
 * The renderer acknowledges each frame once it has painted it. A frame arriving
 * while one is unacknowledged is dropped and counted: the preview is a monitor,
 * so the newest frame is the only one worth showing, and showing it late is
 * worse than not showing the one before it.
 */
export class FramePump {
  private port: FramePort | undefined
  private inFlight = false
  private droppedCount = 0
  private deliveredCount = 0

  get dropped(): number {
    return this.droppedCount
  }

  get delivered(): number {
    return this.deliveredCount
  }

  /**
   * Attach the renderer's port. Frames that arrive before this are dropped and
   * counted, which is the honest reading: nothing was painting them.
   */
  attach(port: FramePort): void {
    this.port = port
    this.inFlight = false
  }

  detach(): void {
    this.port = undefined
    this.inFlight = false
  }

  post(frame: ArrayBuffer): void {
    if (this.port === undefined || this.inFlight) {
      this.droppedCount += 1
      return
    }
    this.inFlight = true
    this.deliveredCount += 1
    this.port.postMessage({ frame, dropped: this.droppedCount, delivered: this.deliveredCount })
  }

  /** The renderer painted the frame in flight; the next one may go. */
  ack(): void {
    this.inFlight = false
  }
}
