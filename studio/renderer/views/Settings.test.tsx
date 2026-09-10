/**
 * @vitest-environment jsdom
 *
 * The mode is offered, written, and honestly described (Plan 0167 Phase 5).
 *
 * The last of those is the one worth a test: ADR-0186's Negative is that in
 * `windowless` the preview stops being by construction the audience's picture,
 * and a surface that did not say so would invite someone to judge a show by a
 * window nobody else can see.
 */
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { Settings } from './Settings'

const setPlayerMode = vi.fn(() => Promise.resolve({ ok: true as const }))

beforeEach(() => {
  setPlayerMode.mockClear()
  // The one bridge, stood in: the panel reaches main through `window.api` and
  // through nothing else, which is what makes a stub this small enough.
  Object.assign(window, { api: { app: { setPlayerMode } } })
})

afterEach(cleanup)

function panel(running: 'windowed' | 'windowless' = 'windowed') {
  render(
    <Settings
      running={running}
      playerPath="/opt/ritmolux"
      playerSource="bundled"
      studioVersion="0.113.0"
      onClose={vi.fn()}
    />,
  )
}

describe('the player mode', () => {
  it('offers both modes and marks the one the player is running', () => {
    panel('windowless')
    expect((screen.getByLabelText(/windowless/) as HTMLInputElement).checked).toBe(true)
    expect((screen.getByLabelText(/^windowed/) as HTMLInputElement).checked).toBe(false)
  })

  it('writes the choice and says a relaunch is what applies it', async () => {
    panel('windowed')
    await act(async () => {
      fireEvent.click(screen.getByLabelText(/windowless/))
    })
    expect(setPlayerMode).toHaveBeenCalledWith('windowless')
    // The mode is read at spawn (ADR-0186), so a control that looked immediate
    // would be a control that appears to do nothing.
    expect(screen.getByText(/reopen the studio/)).toBeDefined()
  })

  it('shows the refusal when the settings file could not be written', async () => {
    setPlayerMode.mockResolvedValueOnce({ ok: false, reason: 'disk is full' } as never)
    panel('windowed')
    await act(async () => {
      fireEvent.click(screen.getByLabelText(/windowless/))
    })
    expect(screen.getByRole('alert').textContent).toContain('disk is full')
  })

  it('does not describe the windowless preview as a show feed', () => {
    panel('windowed')
    const windowless = screen.getByLabelText(/windowless/).closest('label')
    expect(windowless?.textContent).toMatch(/not a feed of what an audience can see/)
    // And the windowed arm says the opposite, so the distinction is on screen
    // rather than only in the ADR.
    expect(screen.getByLabelText(/^windowed/).closest('label')?.textContent).toMatch(
      /copy of what that window draws/,
    )
  })
})

describe('what else the panel states', () => {
  it('names the player it resolved and where it came from', () => {
    panel()
    expect(screen.getByText('/opt/ritmolux')).toBeDefined()
    expect(screen.getByText(/bundled/)).toBeDefined()
  })
})
