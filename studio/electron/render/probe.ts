/**
 * Whether the diffusion sidecar can run on this machine (ADR-0262).
 *
 * The sidecar never ships: it is a script in a checkout and a venv the user
 * built, and it needs CUDA to be worth starting. So the neural switch is
 * offered only once both paths are set, the script is there, and the
 * interpreter's torch reports a CUDA device. Every refusal is one line naming
 * the key to set or what the interpreter said, because the switch is disabled
 * with that line beside it and a bare "unavailable" is a support question.
 */
import { existsSync } from 'node:fs'

import type { DiffusionSettings, ProbeResult } from '@shared/render'

import type { RunTool } from './transcode'

/** What the interpreter is asked: torch imports, and sees a CUDA device. */
export const TORCH_CHECK = 'import torch; print(torch.cuda.is_available())'

const SEE = 'see docs/diffusion-filter.md'

export async function probeDiffusion(
  diffusion: DiffusionSettings | undefined,
  run: RunTool,
  exists: (path: string) => boolean = existsSync,
): Promise<ProbeResult> {
  const script = diffusion?.script
  const python = diffusion?.python
  if (script === undefined) {
    return {
      ready: false,
      reason: 'set render.diffusion.script to tools/sd-filter/sd_filter.py in a checkout',
    }
  }
  if (!exists(script)) {
    return { ready: false, reason: `render.diffusion.script names no file: ${script}` }
  }
  if (python === undefined) {
    return {
      ready: false,
      reason: `set render.diffusion.python to the interpreter of a venv with torch - ${SEE}`,
    }
  }
  let answer: string
  try {
    answer = await run(python, ['-c', TORCH_CHECK])
  } catch (error) {
    return { ready: false, reason: `${python} could not import torch: ${(error as Error).message}` }
  }
  const last = answer.trim().split(/\r?\n/).at(-1)
  if (last === 'True') return { ready: true }
  return { ready: false, reason: `torch reports no CUDA - ${SEE}` }
}
