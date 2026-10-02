/**
 * @vitest-environment jsdom
 *
 * Prompts land on the player's bars, and the strip tells an estimated bar from
 * a fallback one (Plan 0247 Phase 3).
 */
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { BarGrid, Peaks } from '@shared/render'
import type { TimelineEntry } from '@shared/timeline'

import { BarStrip } from './BarStrip'

afterEach(cleanup)

const GRID: BarGrid = {
  fps: '30',
  frames: 400,
  bar_starts: [0, 96, 192, 288],
  bar_locked: [false, true, false, true],
}
const PEAKS: Peaks = { min: [0, -0.5, -1], max: [0, 0.5, 1] }

function strip(timeline: TimelineEntry[], onChange = vi.fn()) {
  const view = render(<BarStrip grid={GRID} peaks={PEAKS} timeline={timeline} onChange={onChange} />)
  // jsdom lays nothing out; the lane is 1000 px wide from x = 100.
  const lane = screen.getByTestId('bar-lane')
  vi.spyOn(lane, 'getBoundingClientRect').mockReturnValue({
    left: 100,
    width: 1000,
    top: 0,
    height: 96,
    right: 1100,
    bottom: 96,
    x: 100,
    y: 0,
    toJSON: () => ({}),
  })
  return { view, onChange }
}

describe('the bar strip', () => {
  it('moves a dragged marker onto a bar start and keeps its prompt', () => {
    const timeline = [
      { at_bar: 1, prompt: 'a vast canyon of luminous rock' },
      { at_bar: 2, prompt: 'a cathedral rose window' },
    ]
    const { view, onChange } = strip(timeline)
    const marker = screen.getByRole('button', { name: 'prompt at bar 1' })
    fireEvent.mouseDown(marker, { button: 0, clientX: 100 })
    // Frame 230 at 2.5 px a frame: between bar 3 (192) and bar 4 (288),
    // nearer bar 3.
    fireEvent.mouseMove(window, { clientX: 100 + 230 * 2.5 })
    fireEvent.mouseUp(window, { clientX: 100 + 230 * 2.5 })

    const moved = onChange.mock.lastCall?.[0] as TimelineEntry[]
    expect(moved).toEqual([
      { at_bar: 2, prompt: 'a cathedral rose window' },
      { at_bar: 3, prompt: 'a vast canyon of luminous rock' },
    ])

    view.rerender(<BarStrip grid={GRID} peaks={PEAKS} timeline={moved} onChange={onChange} />)
    const landed = screen.getByRole('button', { name: 'prompt at bar 3' })
    const bar = screen.getByRole('button', { name: 'bar 3' })
    expect(landed.style.left).toBe('48%')
    expect(landed.style.left).toBe(bar.style.left)
    // The prompt travelled with it.
    expect((screen.getByLabelText(/bar 3/, { selector: 'input' }) as HTMLInputElement).value).toBe(
      'a vast canyon of luminous rock',
    )
  })

  it('leaves a marker where it was when dropped on a bar that already holds one', () => {
    const timeline = [
      { at_bar: 1, prompt: 'one' },
      { at_bar: 3, prompt: 'three' },
    ]
    const { onChange } = strip(timeline)
    fireEvent.mouseDown(screen.getByRole('button', { name: 'prompt at bar 1' }), { button: 0 })
    fireEvent.mouseUp(window, { clientX: 100 + 192 * 2.5 })
    expect(onChange.mock.lastCall?.[0]).toBe(timeline)
  })

  it('draws locked and fallback bars with different classes', () => {
    strip([{ at_bar: 1, prompt: 'one' }])
    const fallback = screen.getByRole('button', { name: 'bar 1' })
    const locked = screen.getByRole('button', { name: 'bar 2, on a downbeat' })
    expect(locked.className).not.toBe(fallback.className)
    expect(screen.getByRole('button', { name: 'bar 4, on a downbeat' }).className).toBe(locked.className)
    expect(screen.getByRole('button', { name: 'bar 3' }).className).toBe(fallback.className)
  })

  it('states the grid under the strip', () => {
    strip([{ at_bar: 1, prompt: 'one' }])
    expect(screen.getByText('4 bars, 2 on a downbeat')).toBeDefined()
  })

  it('adds a prompt on a clicked bar, and removes one', () => {
    const timeline = [{ at_bar: 1, prompt: 'one' }]
    const { onChange } = strip(timeline)
    fireEvent.click(screen.getByRole('button', { name: 'bar 3' }))
    expect(onChange).toHaveBeenLastCalledWith([
      { at_bar: 1, prompt: 'one' },
      { at_bar: 3, prompt: '' },
    ])
    fireEvent.click(screen.getByRole('button', { name: 'remove' }))
    expect(onChange).toHaveBeenLastCalledWith([])
  })
})
