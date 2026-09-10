/**
 * `--capture <file> [--capture-after <ms>]`: save one PNG of the window and
 * quit.
 *
 * The preview's end-to-end path — child, pipe, splitter, port, canvas — has no
 * assertion that reaches the pixels, because the last leg is a compositor. This
 * takes the picture a person would otherwise be asked to describe, so the same
 * evidence can be produced on a machine nobody is sitting at.
 *
 * Unpackaged only. `capturePage` on a window that has never been shown returns
 * an empty image, which is why the delay is a wall-clock wait rather than a
 * frame count: it has to outlast the child's adapter enumeration too.
 */
import { writeFile } from 'node:fs/promises'
import type { BrowserWindow } from 'electron'

export interface CaptureRequest {
  file: string
  afterMs: number
}

const DEFAULT_AFTER_MS = 6000

/** Parse the request out of `process.argv`, or `undefined` when absent. */
export function captureRequest(argv: readonly string[]): CaptureRequest | undefined {
  const at = argv.indexOf('--capture')
  if (at === -1) return undefined
  const file = argv[at + 1]
  if (file === undefined || file.startsWith('--')) return undefined
  const afterAt = argv.indexOf('--capture-after')
  const afterRaw = afterAt === -1 ? undefined : Number(argv[afterAt + 1])
  const afterMs =
    afterRaw !== undefined && Number.isFinite(afterRaw) && afterRaw >= 0
      ? afterRaw
      : DEFAULT_AFTER_MS
  return { file, afterMs }
}

export async function runCapture(window: BrowserWindow, request: CaptureRequest): Promise<void> {
  await new Promise((resolve) => setTimeout(resolve, request.afterMs))
  if (window.isDestroyed()) return
  const image = await window.webContents.capturePage()
  await writeFile(request.file, image.toPNG())
  console.log(`[capture] wrote ${request.file}`)
  window.destroy()
}
