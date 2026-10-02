/**
 * The command lines of a clip render, and the `--bars` call that comes first.
 *
 * Built here and nowhere else, so the pipeline, the render log and the tests
 * read the same argument lists.
 */
import { readFileSync } from 'node:fs'

import { parseBarGrid, type BarGrid, type RenderRequest } from '@shared/render'

import type { RunTool } from './transcode'

/**
 * The encoder's rate-quality setting: `shot`'s `DEFAULT_CRF`, archival rather
 * than shareable.
 */
export const ENCODER_CRF = 18

/**
 * **The canonical encode**, argument for argument what `shot --render
 * --ffmpeg` generates (`shot::render::ffmpeg_args`, whose doc comment says why
 * each one is there), plus `-progress pipe:3`: the encoder's own count of what
 * it has written, on a descriptor of its own. `commands.test.ts` holds the two
 * lists to each other by reading the Rust source.
 */
export function encoderArgs(wav: string, output: string): string[] {
  return [
    '-hide_banner',
    '-nostats',
    '-y',
    '-f',
    'yuv4mpegpipe',
    '-i',
    'pipe:0',
    '-i',
    wav,
    '-map',
    '0:v:0',
    '-map',
    '1:a:0',
    '-c:v',
    'libx264',
    '-preset',
    'medium',
    '-crf',
    String(ENCODER_CRF),
    '-pix_fmt',
    'yuv420p',
    '-color_range',
    'pc',
    '-colorspace',
    'bt709',
    '-color_primaries',
    'bt709',
    '-color_trc',
    'bt709',
    '-x264-params',
    'colorprim=bt709:transfer=bt709',
    '-c:a',
    'aac',
    '-b:a',
    '192k',
    '-shortest',
    '-progress',
    'pipe:3',
    output,
  ]
}

/** `ritmolux --render`: Y4M on stdout. */
export function playerRenderArgs(wav: string, request: RenderRequest): string[] {
  return [
    '--render',
    wav,
    '--preset',
    request.preset,
    '--fps',
    request.fps,
    '--size',
    request.size,
    '--tier',
    request.tier,
  ]
}

/** `ritmolux --bars`: the grid that render draws, written to `out`. */
export function barsArgs(wav: string, fps: string, out: string): string[] {
  return ['--bars', wav, '--fps', fps, '--out', out]
}

/** Run `--bars` and read the grid it wrote. */
export async function readBars(
  run: RunTool,
  player: string,
  wav: string,
  fps: string,
  out: string,
): Promise<BarGrid> {
  try {
    await run(player, barsArgs(wav, fps, out))
  } catch (error) {
    throw new Error(`the player could not count the bars: ${(error as Error).message}`)
  }
  return parseBarGrid(JSON.parse(readFileSync(out, 'utf8')) as unknown)
}
