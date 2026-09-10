/**
 * What travels on the frame port.
 *
 * Shared rather than main-side because both ends of the port are typed from it:
 * the renderer must not reach into `electron/` for a shape, and main must not
 * reach into `renderer/`. The port is the seam and this is its contract.
 */

/** One frame on its way to the renderer, with the counters that describe it. */
export interface FrameMessage {
  /** Tight `width * height * 4` RGBA8 bytes; transferred, not copied. */
  frame: ArrayBuffer
  /** Frames dropped since start because the renderer had not acknowledged. */
  dropped: number
  /** Frames handed to the renderer since start. */
  delivered: number
}

/** What the renderer posts back once it has painted. */
export interface FrameAck {
  ack: true
}
