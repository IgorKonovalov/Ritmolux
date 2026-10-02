/**
 * A job file reopens the render that wrote it (Plan 0247 Phase 5).
 */
import { existsSync, mkdtempSync, readFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import type { RenderRequest } from '@shared/render'

import { jobFile, readJob, writeJob } from './job'

function request(output: string, neural: RenderRequest['neural']): RenderRequest {
  return {
    source: '/home/me/Music/track.flac',
    preset: 'Supernova',
    fps: '30',
    size: '1920x1080',
    tier: 'rich',
    output,
    neural,
  }
}

describe('the job file', () => {
  it.each([
    ['a plain job', null],
    [
      'a neural job',
      {
        profile: 'fast' as const,
        negative: null,
        seed: 1234,
        timeline: [
          { at_bar: 1, prompt: 'a vast canyon of luminous rock' },
          { at_bar: 33, prompt: 'a cathedral rose window in stained glass' },
        ],
      },
    ],
  ])('round-trips %s through write and open', (_case, neural) => {
    const output = join(mkdtempSync(join(tmpdir(), 'rlx-job-')), 'track-supernova.mp4')
    const written = request(output, neural)
    const path = writeJob(written)
    expect(path).toBe(jobFile(output))
    expect(path.endsWith('track-supernova.mp4.render.json')).toBe(true)
    expect(existsSync(`${path}.tmp`)).toBe(false)

    const text = readFileSync(path, 'utf8')
    expect(JSON.parse(text)).toMatchObject({ version: 1, audio: '/home/me/Music/track.flac' })
    expect(readJob(text)).toEqual(written)
  })

  it('refuses a document with an unknown version, naming it', () => {
    const text = JSON.stringify({
      version: 2,
      audio: '/a.flac',
      preset: 'P',
      fps: '30',
      size: '1920x1080',
      tier: 'rich',
      output: '/o.mp4',
      neural: null,
    })
    expect(() => readJob(text)).toThrow('the job file is version 2, and this studio reads version 1')
    expect(() => readJob('{"audio":"/a.flac"}')).toThrow(/version none/)
  })

  it('refuses what is not a job', () => {
    expect(() => readJob('not json')).toThrow(/not JSON/)
    expect(() => readJob('[1]')).toThrow(/not a render job/)
    expect(() =>
      readJob(JSON.stringify({ version: 1, audio: '/a.flac', preset: 'P', fps: '29.97' })),
    ).toThrow(/not one this studio reads/)
  })
})
