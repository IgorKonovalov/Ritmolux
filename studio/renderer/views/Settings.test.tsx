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
const setReducedMotion = vi.fn(() => Promise.resolve({ ok: true as const }))
const onReducedMotion = vi.fn()

beforeEach(() => {
  setPlayerMode.mockClear()
  setReducedMotion.mockClear()
  onReducedMotion.mockClear()
  // The one bridge, stood in: the panel reaches main through `window.api` and
  // through nothing else, which is what makes a stub this small enough.
  Object.assign(window, { api: { app: { setPlayerMode, setReducedMotion } } })
})

afterEach(cleanup)

function panel(running: 'windowed' | 'windowless' = 'windowed', reducedMotion = false) {
  return render(
    <Settings
      running={running}
      playerPath="/opt/ritmolux"
      playerSource="bundled"
      studioVersion="0.113.0"
      reducedMotion={reducedMotion}
      onReducedMotion={onReducedMotion}
      onClose={vi.fn()}
    />,
  )
}

/** The same panel, re-rendered with a different running mode. */
function again(view: ReturnType<typeof panel>, running: 'windowed' | 'windowless'): void {
  view.rerender(
    <Settings
      running={running}
      playerPath="/opt/ritmolux"
      playerSource="bundled"
      studioVersion="0.113.0"
      reducedMotion={false}
      onReducedMotion={onReducedMotion}
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

  it('follows the running mode when the app info arrives after the first render', () => {
    // `running` is one IPC round trip behind the mount. A panel that seeded its
    // state from the first value would show `windowed` on a machine set to
    // `windowless`, and claim a choice nobody made.
    const view = panel('windowed')
    again(view, 'windowless')
    expect((screen.getByLabelText(/windowless/) as HTMLInputElement).checked).toBe(true)
    expect(screen.getByText('The player is running in windowless.')).toBeDefined()
  })

  it('keeps a choice the user made when the info catches up behind it', async () => {
    const view = panel('windowed')
    await act(async () => {
      fireEvent.click(screen.getByLabelText(/windowless/))
    })
    again(view, 'windowed')
    expect((screen.getByLabelText(/windowless/) as HTMLInputElement).checked).toBe(true)
  })

  it('says nothing about saving until something was saved', () => {
    panel('windowed')
    expect(screen.getByText('The player is running in windowed.')).toBeDefined()
    expect(screen.queryByText(/Saved\./)).toBeNull()
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

describe('reduced motion', () => {
  it('shows the value the file holds', () => {
    panel('windowed', true)
    expect((screen.getByLabelText(/reduce motion/) as HTMLInputElement).checked).toBe(true)
  })

  it('writes the choice and applies it once the file took it', async () => {
    panel('windowed', false)
    await act(async () => {
      fireEvent.click(screen.getByLabelText(/reduce motion/))
    })
    expect(setReducedMotion).toHaveBeenCalledWith(true)
    expect(onReducedMotion).toHaveBeenCalledWith(true)
  })

  it('applies nothing when the file refused it, and says so', async () => {
    setReducedMotion.mockResolvedValueOnce({ ok: false, reason: 'read-only' } as never)
    panel('windowed', false)
    await act(async () => {
      fireEvent.click(screen.getByLabelText(/reduce motion/))
    })
    expect(onReducedMotion).not.toHaveBeenCalled()
    expect(screen.getByRole('alert').textContent).toContain('read-only')
  })
})

describe('the render paths', () => {
  it('shows the file values, writes an edit, and clears a key with an empty field', async () => {
    const setSettings = vi.fn(() => Promise.resolve({ ok: true as const, value: null }))
    Object.assign(window, { api: { app: { setPlayerMode, setReducedMotion }, render: { setSettings } } })
    render(
      <Settings
        running="windowed"
        playerPath="/opt/ritmolux"
        playerSource="bundled"
        studioVersion="0.113.0"
        reducedMotion={false}
        onReducedMotion={onReducedMotion}
        render={{ ffmpegPath: '/opt/ffmpeg' }}
        onClose={vi.fn()}
      />,
    )
    const ffmpeg = screen.getByLabelText('ffmpeg') as HTMLInputElement
    const folder = screen.getByLabelText('output folder') as HTMLInputElement
    expect(ffmpeg.value).toBe('/opt/ffmpeg')
    // Absent says what it falls back to rather than showing nothing.
    expect(folder.placeholder).toMatch(/Videos/)

    fireEvent.change(folder, { target: { value: '/data/clips' } })
    const [saveFfmpeg, saveFolder] = screen.getAllByRole('button', { name: 'save' })
    await act(async () => {
      fireEvent.click(saveFolder)
    })
    expect(setSettings).toHaveBeenLastCalledWith({ outputDir: '/data/clips' })

    fireEvent.change(ffmpeg, { target: { value: '' } })
    await act(async () => {
      fireEvent.click(saveFfmpeg)
    })
    expect(setSettings).toHaveBeenLastCalledWith({ ffmpegPath: null })
    expect(ffmpeg.value).toBe('')

    // The diffusion paths are the nested keys, written by their dotted names.
    const python = screen.getByLabelText('diffusion python') as HTMLInputElement
    expect(python.placeholder).toMatch(/no neural renders/)
    fireEvent.change(python, { target: { value: '/venv/bin/python' } })
    await act(async () => {
      fireEvent.click(screen.getAllByRole('button', { name: 'save' })[2])
    })
    expect(setSettings).toHaveBeenLastCalledWith({ 'diffusion.python': '/venv/bin/python' })
  })
})

describe('what else the panel states', () => {
  it('names the player it resolved and where it came from', () => {
    panel()
    expect(screen.getByText('/opt/ritmolux')).toBeDefined()
    expect(screen.getByText(/bundled/)).toBeDefined()
  })
})
