/**
 * What travels on the frame port, and how its bytes become paintable.
 *
 * Shared rather than main-side because both ends of the port are typed from it:
 * the renderer must not reach into `electron/` for a shape, and main must not
 * reach into `renderer/`. The port is the seam and this is its contract.
 */
import type { PixelFormat } from './protocol'

/** One frame on its way to the renderer, with the counters that describe it. */
export interface FrameMessage {
  /**
   * Tight `width * height * 4` bytes in the channel order the `stream` event
   * named — which is not always RGBA (ADR-0187). Nothing on this side of the
   * port converts them; `rgbaPixels` does, at the paint.
   */
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

/**
 * The frame's bytes as `ImageData` takes them, which is RGBA and only RGBA.
 *
 * A `bgra8` frame has its first and third bytes swapped **in place**: the buffer
 * was cut out of the pipe for this port and nothing else holds it, so a second
 * allocation per frame would buy nothing. `rgba8` is returned untouched, which
 * is what makes the swap apply to exactly one of the two orders.
 *
 * Measured at the preview's fixed 640x360: 0.33 ms to view the buffer, 0.55 ms
 * to view and swap it — 0.22 ms of swap per frame, or 3.7 % of one core at
 * 165 frames a second. A WebGL texture upload with the swizzle in a fragment
 * shader would save that 0.22 ms and cost a GL context, a program and a
 * disposal path in a component that currently has none.
 */
export function rgbaPixels(frame: ArrayBuffer, format: PixelFormat): Uint8ClampedArray {
  const pixels = new Uint8ClampedArray(frame)
  if (format === 'rgba8') return pixels
  for (let i = 0; i + 3 < pixels.length; i += 4) {
    const blue = pixels[i]
    pixels[i] = pixels[i + 2]
    pixels[i + 2] = blue
  }
  return pixels
}
