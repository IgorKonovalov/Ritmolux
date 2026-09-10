/**
 * @vitest-environment jsdom
 *
 * The preview paints the order it was told, and refuses one it cannot name
 * (Plan 0167 Phase 4).
 *
 * jsdom has no canvas implementation and therefore no `ImageData` and no 2D
 * context, so both are stood in here — the fake context records what was handed
 * to `putImageData`, which is the pixels the canvas would hold. Reading them
 * back is the whole assertion: a swizzle applied to both orders, or to neither,
 * passes every test that only checks a frame arrived.
 */
import { act, cleanup, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { FRAME_PORT_SENTINEL } from '@shared/ipc-channels'
import type { FrameMessage } from '@shared/frames'
import type { StreamEvent } from '@shared/protocol'

import { Preview } from './Preview'

/** What `putImageData` was handed, in order. */
let painted: { data: Uint8ClampedArray; width: number; height: number }[] = []

class FakeImageData {
  constructor(
    readonly data: Uint8ClampedArray,
    readonly width: number,
    readonly height: number,
  ) {}
}

beforeEach(() => {
  painted = []
  vi.stubGlobal('ImageData', FakeImageData)
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
    putImageData: (image: FakeImageData) =>
      painted.push({ data: image.data, width: image.width, height: image.height }),
  } as unknown as CanvasRenderingContext2D)
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
})

const stream = (format: string, width = 2, height = 1): StreamEvent =>
  ({ v: 1, ev: 'stream', width, height, fps: 30, format }) as StreamEvent

/** One pixel per entry, written in the order the argument names. */
function frameOf(...pixels: number[][]): ArrayBuffer {
  const bytes = new Uint8ClampedArray(pixels.flat())
  return bytes.buffer
}

/**
 * Render the preview against a live port, push one frame, and return what the
 * canvas was handed.
 *
 * The port is dispatched before the render because `framePort` holds whatever
 * arrived last and answers a later asker with it.
 */
async function paint(event: StreamEvent, frame: ArrayBuffer): Promise<void> {
  const channel = new MessageChannel()
  window.dispatchEvent(
    new MessageEvent('message', {
      data: FRAME_PORT_SENTINEL,
      source: window as unknown as MessageEventSource,
      ports: [channel.port2],
    }),
  )
  render(<Preview stream={event} onStats={vi.fn()} />)
  const message: FrameMessage = { frame, dropped: 0, delivered: 1 }
  // Inside `act` because painting sets state; the port delivers on a task of
  // its own, so one turn of the loop is what the wrapper has to wait for.
  await act(async () => {
    channel.port1.postMessage(message)
    await new Promise((resolve) => setTimeout(resolve, 0))
  })
}

describe('the channel order the stream announced', () => {
  it('paints a bgra8 frame and an rgba8 frame of one picture to the same colours', async () => {
    // Two spellings of the same two pixels: opaque orange, then opaque blue.
    await paint(stream('rgba8'), frameOf([255, 128, 0, 255], [0, 0, 255, 255]))
    await paint(stream('bgra8'), frameOf([0, 128, 255, 255], [255, 0, 0, 255]))

    expect(painted).toHaveLength(2)
    expect([...painted[1].data]).toEqual([...painted[0].data])
    // And not vacuously: the picture really is orange-then-blue in RGBA.
    expect([...painted[0].data]).toEqual([255, 128, 0, 255, 0, 0, 255, 255])
  })

  it('leaves an rgba8 frame alone, so the swap applies to exactly one order', async () => {
    const bytes = [1, 2, 3, 4, 5, 6, 7, 8]
    await paint(stream('rgba8'), frameOf(bytes.slice(0, 4), bytes.slice(4)))
    expect([...painted[0].data]).toEqual(bytes)
  })

  it('refuses an order it cannot name rather than painting a guess', () => {
    render(<Preview stream={stream('rgbx9')} onStats={vi.fn()} />)
    // Visible, and not a blank canvas: the treatment an unknown `hello`
    // version already gets.
    expect(screen.getByRole('alert').textContent).toContain('rgbx9')
    expect(screen.queryByLabelText("The player's preview picture")).toBeNull()
    expect(painted).toHaveLength(0)
  })
})

describe('what the preview reads off the stream', () => {
  it('takes its geometry from the event and hard-codes no size', async () => {
    const wide = stream('rgba8', 3, 2)
    await paint(wide, new ArrayBuffer(3 * 2 * 4))
    const canvas = screen.getByLabelText("The player's preview picture") as HTMLCanvasElement
    expect([canvas.width, canvas.height]).toEqual([3, 2])
    expect([painted[0].width, painted[0].height]).toEqual([3, 2])
  })

  it('paints nothing when the bytes are not the frame the event described', async () => {
    await paint(stream('rgba8', 2, 2), new ArrayBuffer(4))
    expect(painted).toHaveLength(0)
  })

  it('says it is waiting rather than showing an empty box before the stream', () => {
    render(<Preview stream={undefined} onStats={vi.fn()} />)
    expect(screen.queryByLabelText("The player's preview picture")).toBeNull()
    expect(screen.getByText(/waiting for the player/)).toBeDefined()
  })
})
