/**
 * The strip's waveform is the file's own extremes (Plan 0247 Phase 3).
 */
import { describe, expect, it } from 'vitest'

import { peaksOf } from './peaks'

/**
 * A 16-bit PCM WAV of `channels` channels at `rate`, with a `LIST` chunk before
 * `data` the way ffmpeg writes one, from one sample function per frame.
 */
function wav(rate: number, channels: number, frames: number, sample: (frame: number) => number): Uint8Array {
  const list = new TextEncoder().encode('INFOISFT\x05\x00\x00\x00Lavf\x00\x00')
  const data = frames * channels * 2
  const size = 12 + 8 + 16 + 8 + list.length + 8 + data
  const bytes = new Uint8Array(size)
  const view = new DataView(bytes.buffer)
  const put = (at: number, text: string): void => {
    for (let i = 0; i < text.length; i += 1) bytes[at + i] = text.charCodeAt(i)
  }
  put(0, 'RIFF')
  view.setUint32(4, size - 8, true)
  put(8, 'WAVE')
  put(12, 'fmt ')
  view.setUint32(16, 16, true)
  view.setUint16(20, 1, true)
  view.setUint16(22, channels, true)
  view.setUint32(24, rate, true)
  view.setUint32(28, rate * channels * 2, true)
  view.setUint16(32, channels * 2, true)
  view.setUint16(34, 16, true)
  put(36, 'LIST')
  view.setUint32(40, list.length, true)
  bytes.set(list, 44)
  let at = 44 + list.length
  put(at, 'data')
  view.setUint32(at + 4, data, true)
  at += 8
  for (let frame = 0; frame < frames; frame += 1) {
    for (let channel = 0; channel < channels; channel += 1) {
      view.setInt16(at, sample(frame), true)
      at += 2
    }
  }
  return bytes
}

describe('the peaks of a track', () => {
  it('reads a second of silence as zero and a full-scale square as ±1, within one column of the boundary', () => {
    const rate = 8000
    // 1 s of silence, then 1 s of a 100 Hz full-scale square wave.
    const bytes = wav(rate, 2, 2 * rate, (frame) =>
      frame < rate ? 0 : Math.floor(frame / 40) % 2 === 0 ? 32767 : -32768,
    )
    const columns = 200
    const { min, max } = peaksOf(bytes, columns)
    expect(min).toHaveLength(columns)
    const boundary = columns / 2
    for (let column = 0; column < columns; column += 1) {
      if (Math.abs(column - boundary) <= 1) continue
      if (column < boundary) {
        expect([min[column], max[column]]).toEqual([0, 0])
      } else {
        expect([min[column], max[column]]).toEqual([-1, 1])
      }
    }
  })

  it('takes the extremes across every channel', () => {
    const bytes = wav(1000, 2, 1000, () => 0)
    const view = new DataView(bytes.buffer)
    // The right channel of the last frame, which is the file's last two bytes.
    view.setInt16(bytes.length - 2, -16384, true)
    const { min } = peaksOf(bytes, 10)
    expect(min[9]).toBe(-0.5)
  })

  it('refuses a file that is not 16-bit PCM', () => {
    const bytes = wav(1000, 1, 10, () => 0)
    new DataView(bytes.buffer).setUint16(34, 24, true)
    expect(() => peaksOf(bytes)).toThrow(/16-bit PCM/)
    expect(() => peaksOf(new Uint8Array(8))).toThrow(/not a WAV/)
  })
})
