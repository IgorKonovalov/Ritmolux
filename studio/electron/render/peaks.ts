/**
 * The waveform the bar strip draws: per column, the lowest and highest sample
 * across every channel, from the session's transcoded WAV (ADR-0262).
 *
 * A drawing of the file, not an estimate of anything the engine analyses —
 * the bars over it are the player's own `--bars`. The reader takes the one
 * shape the transcode writes, 16-bit PCM, and walks the RIFF chunks rather than
 * assuming a 44-byte header, because ffmpeg writes a `LIST` chunk before `data`.
 */
import { readFile } from 'node:fs/promises'

import type { Peaks } from '@shared/render'

/** Columns the strip is drawn at; the drawing scales to the strip's width. */
export const PEAK_COLUMNS = 1200

interface PcmView {
  channels: number
  /** The `data` chunk's samples, little-endian `i16`, interleaved. */
  samples: DataView
}

function readPcm(bytes: Uint8Array): PcmView {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  const tag = (at: number): string => String.fromCharCode(...bytes.subarray(at, at + 4))
  if (bytes.byteLength < 12 || tag(0) !== 'RIFF' || tag(8) !== 'WAVE') {
    throw new Error('the transcoded track is not a WAV file')
  }
  let channels: number | undefined
  let at = 12
  while (at + 8 <= bytes.byteLength) {
    const id = tag(at)
    const size = view.getUint32(at + 4, true)
    const body = at + 8
    if (id === 'fmt ') {
      const format = view.getUint16(body, true)
      const bits = view.getUint16(body + 14, true)
      if (format !== 1 || bits !== 16) throw new Error('the transcoded track is not 16-bit PCM')
      channels = view.getUint16(body + 2, true)
    } else if (id === 'data') {
      if (channels === undefined || channels === 0) throw new Error('the WAV has no format before its data')
      const length = Math.min(size, bytes.byteLength - body)
      return { channels, samples: new DataView(bytes.buffer, bytes.byteOffset + body, length) }
    }
    // Chunks are word-aligned: an odd size carries a pad byte.
    at = body + size + (size % 2)
  }
  throw new Error('the WAV has no data chunk')
}

/** Peaks of 16-bit PCM WAV bytes, at `columns` columns, in [-1, 1]. */
export function peaksOf(bytes: Uint8Array, columns = PEAK_COLUMNS): Peaks {
  const { channels, samples } = readPcm(bytes)
  const frames = Math.floor(samples.byteLength / 2 / channels)
  const count = Math.max(1, Math.min(columns, frames))
  const min = new Array<number>(count).fill(0)
  const max = new Array<number>(count).fill(0)
  for (let column = 0; column < count; column += 1) {
    const first = Math.floor((column * frames) / count)
    const end = Math.floor(((column + 1) * frames) / count)
    let low = 0
    let high = 0
    for (let frame = first; frame < end; frame += 1) {
      for (let channel = 0; channel < channels; channel += 1) {
        const sample = samples.getInt16((frame * channels + channel) * 2, true)
        if (sample < low) low = sample
        if (sample > high) high = sample
      }
    }
    // Each side over its own full scale, so a full-scale square reads ±1.
    min[column] = low / 32768
    max[column] = high / 32767
  }
  return { min, max }
}

/** The peaks of a WAV file on disk. */
export async function peaksOfFile(path: string, columns = PEAK_COLUMNS): Promise<Peaks> {
  return peaksOf(await readFile(path), columns)
}
