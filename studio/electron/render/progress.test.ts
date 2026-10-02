/**
 * The encoder's progress reads as a frame count that only grows, and as done
 * only on `progress=end` (Plan 0247 Phase 2).
 */
import { describe, expect, it } from 'vitest'

import { LineTail, ProgressParser } from './progress'

/**
 * Three blocks of `ffmpeg -progress pipe:3` from a 1080p30 x264 encode, the
 * field set ffmpeg 6 and 7 print. The second block repeats the first's frame,
 * which the encoder does while x264's lookahead fills.
 */
const TRANSCRIPT = [
  'frame=0',
  'fps=0.00',
  'stream_0_0_q=0.0',
  'bitrate=N/A',
  'total_size=48',
  'out_time_us=N/A',
  'out_time_ms=N/A',
  'out_time=N/A',
  'dup_frames=0',
  'drop_frames=0',
  'speed=N/A',
  'progress=continue',
  'frame=38',
  'fps=37.92',
  'stream_0_0_q=28.0',
  'bitrate=   0.2kbits/s',
  'total_size=48',
  'out_time_us=0',
  'out_time_ms=0',
  'out_time=00:00:00.000000',
  'dup_frames=0',
  'drop_frames=0',
  'speed=   0x',
  'progress=continue',
  'frame=38',
  'fps=19.00',
  'progress=continue',
  'frame=91',
  'fps=30.31',
  'stream_0_0_q=28.0',
  'bitrate=2310.4kbits/s',
  'total_size=524336',
  'out_time_us=1816000',
  'out_time_ms=1816000',
  'out_time=00:00:01.816000',
  'dup_frames=0',
  'drop_frames=0',
  'speed=0.605x',
  'progress=continue',
  'frame=120',
  'fps=29.87',
  'stream_0_0_q=-1.0',
  'bitrate=2655.9kbits/s',
  'total_size=1328124',
  'out_time_us=4000000',
  'out_time_ms=4000000',
  'out_time=00:00:04.000000',
  'dup_frames=0',
  'drop_frames=0',
  'speed=0.996x',
  'progress=end',
  '',
].join('\n')

/** Cut `text` into chunks of `size` characters, as a pipe might deliver it. */
function chunks(text: string, size: number): string[] {
  const out: string[] = []
  for (let at = 0; at < text.length; at += size) out.push(text.slice(at, at + size))
  return out
}

describe('the encoder progress parser', () => {
  it.each([1, 3, 7, 64, TRANSCRIPT.length])(
    'reads a monotone frame count in chunks of %i characters',
    (size) => {
      const parser = new ProgressParser()
      const seen: number[] = []
      const doneAt: number[] = []
      for (const chunk of chunks(TRANSCRIPT, size)) {
        if (parser.push(chunk)) seen.push(parser.frame)
        if (parser.done) doneAt.push(parser.frame)
      }
      // Every reading at least the one before; a line cut mid-number is never
      // read as the shorter number.
      for (let i = 1; i < seen.length; i += 1) expect(seen[i]).toBeGreaterThan(seen[i - 1])
      expect(seen).not.toContain(1)
      expect(seen).not.toContain(9)
      expect(parser.frame).toBe(120)
      // Done arrives with the last block and not before it.
      expect(doneAt[0]).toBe(120)
    },
  )

  it('is not done while every block says continue', () => {
    const parser = new ProgressParser()
    parser.push(TRANSCRIPT.slice(0, TRANSCRIPT.indexOf('progress=end')))
    expect(parser.frame).toBe(120)
    expect(parser.done).toBe(false)
  })

  it('ignores a count that goes backwards', () => {
    const parser = new ProgressParser()
    parser.push('frame=50\nprogress=continue\nframe=12\nprogress=continue\n')
    expect(parser.frame).toBe(50)
  })

  it('is not done on a progress line that is not end', () => {
    const parser = new ProgressParser()
    parser.push('frame=5\nprogress=endless\n')
    expect(parser.done).toBe(false)
  })
})

describe('a child stderr tail', () => {
  it('keeps the last lines, the unterminated one included', () => {
    const tail = new LineTail(3)
    tail.push('one\ntwo\r\nthree\nfo')
    tail.push('ur\nfive')
    expect(tail.lines()).toEqual(['three', 'four', 'five'])
  })

  it('hands every whole line to its listener', () => {
    const lines: string[] = []
    const tail = new LineTail(1, (line) => lines.push(line))
    tail.push('sd-filter: 10 frames\nsd-filter: 20 fr')
    tail.push('ames\n')
    expect(lines).toEqual(['sd-filter: 10 frames', 'sd-filter: 20 frames'])
  })
})
