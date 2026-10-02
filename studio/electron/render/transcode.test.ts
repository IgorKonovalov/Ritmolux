/**
 * The session cache of transcoded tracks (Plan 0247 Phase 2).
 */
import { existsSync, mkdtempSync, utimesSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { describe, expect, it, vi } from 'vitest'

import { cacheKey, TranscodeCache, transcodeArgs } from './transcode'

function scratch(): string {
  return mkdtempSync(join(tmpdir(), 'rlx-transcode-'))
}

describe('the cache key', () => {
  it('moves with the path, the size and the modification time', () => {
    const base = cacheKey('/m/a.flac', 100, 1_000)
    expect(cacheKey('/m/a.flac', 100, 1_000)).toBe(base)
    expect(cacheKey('/m/b.flac', 100, 1_000)).not.toBe(base)
    expect(cacheKey('/m/a.flac', 101, 1_000)).not.toBe(base)
    expect(cacheKey('/m/a.flac', 100, 2_000)).not.toBe(base)
  })
})

describe('the transcode', () => {
  it('asks for stereo 16-bit PCM and keeps the sample rate', () => {
    const args = transcodeArgs('/m/a.mp3', '/c/k.wav')
    expect(args).toContain('pcm_s16le')
    expect(args.slice(args.indexOf('-ac'), args.indexOf('-ac') + 2)).toEqual(['-ac', '2'])
    expect(args).not.toContain('-ar')
    expect(args.at(-1)).toBe('/c/k.wav')
  })

  it('transcodes a track once per session, and again once it changed', async () => {
    const dir = scratch()
    const source = join(dir, 'track.flac')
    writeFileSync(source, 'flac bytes')
    const run = vi.fn((_command: string, args: string[]) => {
      writeFileSync(args.at(-1) as string, 'RIFF')
      return Promise.resolve('')
    })
    const cache = new TranscodeCache(join(dir, 'cache'), () => '/opt/ffmpeg', run)

    const first = await cache.wavFor(source)
    expect(await cache.wavFor(source)).toBe(first)
    expect(run).toHaveBeenCalledTimes(1)
    expect(run.mock.calls[0][0]).toBe('/opt/ffmpeg')
    expect(existsSync(first)).toBe(true)

    utimesSync(source, new Date(), new Date(Date.now() + 60_000))
    expect(await cache.wavFor(source)).not.toBe(first)
    expect(run).toHaveBeenCalledTimes(2)
  })

  it('keeps no partial file and no entry when ffmpeg fails', async () => {
    const dir = scratch()
    const source = join(dir, 'track.mp3')
    writeFileSync(source, 'mp3 bytes')
    const run = vi.fn(() => Promise.reject(new Error('Invalid data found when processing input')))
    const cache = new TranscodeCache(join(dir, 'cache'), () => 'ffmpeg', run)
    await expect(cache.wavFor(source)).rejects.toThrow(/could not transcode the track: Invalid data/)
    await expect(cache.wavFor(source)).rejects.toThrow()
    expect(run).toHaveBeenCalledTimes(2)
  })

  it('is emptied when cleared', async () => {
    const dir = scratch()
    const source = join(dir, 'track.wav')
    writeFileSync(source, 'wav bytes')
    const cache = new TranscodeCache(join(dir, 'cache'), () => 'ffmpeg', (_c, args) => {
      writeFileSync(args.at(-1) as string, 'RIFF')
      return Promise.resolve('')
    })
    const wav = await cache.wavFor(source)
    cache.clear()
    expect(existsSync(wav)).toBe(false)
    expect(existsSync(join(dir, 'cache'))).toBe(false)
  })

  it('refuses a track that is not there, naming why', async () => {
    const cache = new TranscodeCache(join(scratch(), 'cache'), () => 'ffmpeg', vi.fn())
    await expect(cache.wavFor('/nowhere/track.flac')).rejects.toThrow(/cannot be read/)
  })
})
