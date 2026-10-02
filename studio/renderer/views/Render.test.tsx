/**
 * @vitest-environment jsdom
 *
 * The neural switch follows the readiness probe (Plan 0247 Phase 4): each of
 * its outcomes is a state of its own, with its own line beside the switch.
 */
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ProbeResult } from '@shared/render'

import { Render } from './Render'

afterEach(cleanup)

async function openWith(...answers: ProbeResult[]) {
  const probe = vi.fn()
  for (const answer of answers) probe.mockResolvedValueOnce(answer)
  Object.assign(window, {
    api: {
      render: {
        onEvent: () => () => undefined,
        probe,
        prepare: vi.fn(),
        suggestOutput: vi.fn(),
      },
    },
  })
  await act(async () => {
    render(<Render roster={['Gyre']} active="Gyre" hidden={false} onClose={vi.fn()} />)
  })
  return probe
}

const neuralSwitch = (): HTMLInputElement => screen.getByLabelText('neural') as HTMLInputElement

describe('the neural switch', () => {
  it('is disabled, naming the key, when no script is set', async () => {
    await openWith({
      ready: false,
      reason: 'set render.diffusion.script to tools/sd-filter/sd_filter.py in a checkout',
    })
    expect(neuralSwitch().disabled).toBe(true)
    expect(screen.getByText(/set render\.diffusion\.script/)).toBeDefined()
  })

  it('is disabled, pointing at the docs, when torch sees no CUDA', async () => {
    await openWith({ ready: false, reason: 'torch reports no CUDA - see docs/diffusion-filter.md' })
    expect(neuralSwitch().disabled).toBe(true)
    expect(screen.getByText('torch reports no CUDA - see docs/diffusion-filter.md')).toBeDefined()
  })

  it('is enabled, and says why, when the sidecar is ready', async () => {
    await openWith({ ready: true })
    expect(neuralSwitch().disabled).toBe(false)
    expect(screen.getByText(/sidecar is ready/)).toBeDefined()
    fireEvent.click(neuralSwitch())
    expect(neuralSwitch().checked).toBe(true)
    expect(screen.getByLabelText('profile')).toBeDefined()
  })

  it('asks once, and again only on re-check', async () => {
    const probe = await openWith(
      { ready: false, reason: 'torch reports no CUDA - see docs/diffusion-filter.md' },
      { ready: true },
    )
    expect(probe).toHaveBeenCalledTimes(1)
    expect(probe).toHaveBeenLastCalledWith(false)
    await act(async () => {
      fireEvent.click(screen.getByRole('button', { name: 're-check' }))
    })
    expect(probe).toHaveBeenCalledTimes(2)
    expect(probe).toHaveBeenLastCalledWith(true)
    expect(neuralSwitch().disabled).toBe(false)
  })
})
