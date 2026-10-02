/**
 * The readiness probe's outcomes are distinct, and each carries its own line
 * (Plan 0247 Phase 4).
 */
import { describe, expect, it, vi } from 'vitest'

import { probeDiffusion, TORCH_CHECK } from './probe'

const PATHS = { python: '/venv/bin/python', script: '/src/tools/sd-filter/sd_filter.py' }

describe('the diffusion probe', () => {
  it('names the key when no script is set, and runs nothing', async () => {
    const run = vi.fn()
    const result = await probeDiffusion({ python: PATHS.python }, run)
    expect(result).toEqual({ ready: false, reason: expect.stringContaining('render.diffusion.script') })
    expect(run).not.toHaveBeenCalled()
  })

  it('names the path when the script is not there', async () => {
    const result = await probeDiffusion(PATHS, vi.fn(), () => false)
    expect(result).toEqual({
      ready: false,
      reason: 'render.diffusion.script names no file: /src/tools/sd-filter/sd_filter.py',
    })
  })

  it('names the interpreter key when only the script is set', async () => {
    const result = await probeDiffusion({ script: PATHS.script }, vi.fn(), () => true)
    expect(result).toEqual({ ready: false, reason: expect.stringContaining('render.diffusion.python') })
  })

  it('says torch sees no CUDA when it prints False', async () => {
    const run = vi.fn(() => Promise.resolve('False\n'))
    const result = await probeDiffusion(PATHS, run, () => true)
    expect(result).toEqual({ ready: false, reason: 'torch reports no CUDA - see docs/diffusion-filter.md' })
    expect(run).toHaveBeenCalledWith(PATHS.python, ['-c', TORCH_CHECK])
  })

  it('quotes the interpreter when torch does not import', async () => {
    const run = vi.fn(() => Promise.reject(new Error("ModuleNotFoundError: No module named 'torch'")))
    const result = await probeDiffusion(PATHS, run, () => true)
    expect(result).toEqual({
      ready: false,
      reason: "/venv/bin/python could not import torch: ModuleNotFoundError: No module named 'torch'",
    })
  })

  it('is ready when torch sees CUDA', async () => {
    const result = await probeDiffusion(PATHS, () => Promise.resolve('True\n'), () => true)
    expect(result).toEqual({ ready: true })
  })
})
