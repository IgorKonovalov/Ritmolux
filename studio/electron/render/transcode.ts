/**
 * The track as the player can read it: a 16-bit PCM WAV in a session cache
 * (ADR-0262).
 *
 * The player's reader takes 16-bit PCM WAV and nothing else, and the render
 * already needs `ffmpeg` for its output, so that `ffmpeg` transcodes whatever
 * the user picked. One WAV per source then feeds `--bars`, `--render` and the
 * encoder's audio input, so all three read the same samples.
 *
 * The cache lives in `userData/render-cache/` and is emptied when the studio
 * starts and when it quits: it is a second copy of every track touched in the
 * session, about 42 MB for four minutes of 44.1 kHz stereo, and nothing reads
 * it across sessions. The key covers the source's path, size and modification
 * time, so a file replaced under the same name is transcoded again.
 */
import { execFile } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdirSync, renameSync, rmSync, statSync } from 'node:fs'
import { join } from 'node:path'

/** What a cache entry is named by. */
export function cacheKey(source: string, size: number, mtimeMs: number): string {
  return createHash('sha256')
    .update(`${source}\0${size}\0${Math.trunc(mtimeMs)}`)
    .digest('hex')
    .slice(0, 24)
}

/**
 * The transcode: stereo, 16-bit PCM, the source's own sample rate kept.
 *
 * `-vn` drops a cover image an MP3 or FLAC carries, and `-map_metadata -1`
 * the tags, neither of which a WAV holds.
 */
export function transcodeArgs(source: string, out: string): string[] {
  return [
    '-hide_banner',
    '-nostats',
    '-y',
    '-i',
    source,
    '-vn',
    '-map_metadata',
    '-1',
    '-c:a',
    'pcm_s16le',
    '-ac',
    '2',
    out,
  ]
}

/** Run one short command, resolving with its stdout or rejecting with its last words. */
export type RunTool = (command: string, args: string[]) => Promise<string>

/** Lines of a failed tool's stderr quoted in its refusal. */
const QUOTED_LINES = 4

export const runTool: RunTool = (command, args) =>
  new Promise((resolve, reject) => {
    execFile(
      command,
      args,
      { windowsHide: true, maxBuffer: 16 * 1024 * 1024 },
      (error, stdout, stderr) => {
        if (error) {
          const said = String(stderr)
            .split(/\r?\n/)
            .filter((line) => line.trim() !== '')
            .slice(-QUOTED_LINES)
            .join(' | ')
          reject(new Error(said === '' ? error.message : `${error.message.split('\n')[0]}: ${said}`))
          return
        }
        resolve(String(stdout))
      },
    )
  })

export class TranscodeCache {
  private readonly entries = new Map<string, Promise<string>>()

  constructor(
    private readonly dir: string,
    private readonly ffmpeg: () => string,
    private readonly run: RunTool = runTool,
  ) {
    this.clear()
  }

  /** The cache directory, which also holds each source's `--bars` answers. */
  get directory(): string {
    return this.dir
  }

  /** The cached WAV for `source`, transcoding it the first time. */
  wavFor(source: string): Promise<string> {
    let stat
    try {
      stat = statSync(source)
    } catch (error) {
      return Promise.reject(new Error(`the track cannot be read: ${(error as Error).message}`))
    }
    const key = cacheKey(source, stat.size, stat.mtimeMs)
    let entry = this.entries.get(key)
    if (entry === undefined) {
      entry = this.transcode(source, key)
      // A failed transcode is not kept: the user may install ffmpeg or fix the
      // path in settings and try the same track again.
      entry.catch(() => this.entries.delete(key))
      this.entries.set(key, entry)
    }
    return entry
  }

  /** Remove every cached file. */
  clear(): void {
    this.entries.clear()
    rmSync(this.dir, { recursive: true, force: true })
  }

  private async transcode(source: string, key: string): Promise<string> {
    mkdirSync(this.dir, { recursive: true })
    const out = join(this.dir, `${key}.wav`)
    // Written under another name and renamed, so a transcode cut short is never
    // the file a later render reads.
    const partial = join(this.dir, `${key}.partial.wav`)
    try {
      await this.run(this.ffmpeg(), transcodeArgs(source, partial))
    } catch (error) {
      rmSync(partial, { force: true })
      throw new Error(`ffmpeg could not transcode the track: ${(error as Error).message}`)
    }
    renameSync(partial, out)
    return out
  }
}
