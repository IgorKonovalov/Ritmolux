/**
 * @vitest-environment jsdom
 *
 * The drag is live and the write is not (Plan 0159 Phase 6).
 *
 * The two halves of one gesture answer to different things: every step of a
 * drag is a datagram the player applies on its next frame, and only the release
 * touches the disk. Getting that backwards would write a file per slider step,
 * which the watcher would answer with a reload per step.
 */
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ParamRoster } from '@shared/schema'
import type { Binding } from '@shared/toml'

import { ParamPanel } from './ParamPanel'

afterEach(cleanup)

const ROSTERS: ParamRoster[] = [
  {
    name: 'fragment_field',
    params: [
      { name: 'warp', default: 0.4, range: [0, 1.5], doc: 'Amplitude of the fold.' },
      { name: 'drift', default: 1, range: [0, 4], doc: 'How fast it turns.' },
    ],
  },
]

const BINDINGS: Binding[] = [
  { kind: 'const', name: 'warp', value: 0.4, line: 5, quote: '"' },
  { kind: 'expr', name: 'drift', text: 'bass * 2', line: 6 },
]

function panel(overrides: Partial<Parameters<typeof ParamPanel>[0]> = {}) {
  const onDrag = vi.fn()
  const onCommit = vi.fn()
  render(
    <ParamPanel
      rosters={ROSTERS}
      bindings={BINDINGS}
      writable
      onDrag={onDrag}
      onCommit={onCommit}
      {...overrides}
    />,
  )
  return { onDrag, onCommit }
}

describe('a drag', () => {
  it('sends one action per step and writes nothing until the release', () => {
    const { onDrag, onCommit } = panel()
    const slider = screen.getByLabelText('warp')

    for (const value of ['0.5', '0.6', '0.7']) {
      fireEvent.change(slider, { target: { value } })
    }

    expect(onDrag).toHaveBeenCalledTimes(3)
    expect(onDrag).toHaveBeenNthCalledWith(1, 'warp', 0.5)
    expect(onDrag).toHaveBeenNthCalledWith(3, 'warp', 0.7)
    // The whole point: three steps, no file touched.
    expect(onCommit).not.toHaveBeenCalled()

    fireEvent.pointerUp(slider)
    expect(onCommit).toHaveBeenCalledTimes(1)
    expect(onCommit).toHaveBeenCalledWith('warp', 0.7)
  })

  it('writes on a key release too, so the keyboard is not a second-class control', () => {
    const { onCommit } = panel()
    const slider = screen.getByLabelText('warp')
    fireEvent.change(slider, { target: { value: '0.9' } })
    fireEvent.keyUp(slider, { key: 'ArrowRight' })
    expect(onCommit).toHaveBeenCalledWith('warp', 0.9)
  })

  it('moves the picture but writes nothing when there is no file behind the preset', () => {
    const { onDrag, onCommit } = panel({ writable: false })
    const slider = screen.getByLabelText('warp')
    fireEvent.change(slider, { target: { value: '1.1' } })
    fireEvent.pointerUp(slider)
    // The embedded set has no file. The override still moves the picture, and
    // that is the honest offer — a disabled slider would say the parameter
    // cannot move, which is false.
    expect(onDrag).toHaveBeenCalledWith('warp', 1.1)
    expect(onCommit).not.toHaveBeenCalled()
  })
})

describe('what each binding renders as', () => {
  it('gives a constant a slider at the value the file holds', () => {
    panel()
    expect(screen.getByLabelText('warp')).toHaveProperty('value', '0.4')
  })

  it('shows an expression and offers no slider for it', () => {
    panel()
    expect(screen.getByText('bass * 2')).toBeDefined()
    expect(screen.queryByLabelText('drift')).toBeNull()
  })

  it('offers a slider at the engine default for a parameter the preset does not bind', () => {
    const { onDrag } = panel({ bindings: [] })
    const slider = screen.getByLabelText('drift')
    expect(slider).toHaveProperty('value', '1')
    fireEvent.change(slider, { target: { value: '2' } })
    expect(onDrag).toHaveBeenCalledWith('drift', 2)
  })

  it('builds every row from the schema and none from a list of its own', () => {
    panel({
      rosters: [{ name: 'invented', params: [{ name: 'x', default: 0, range: [0, 1], doc: '' }] }],
      bindings: [],
    })
    expect(screen.getByLabelText('x')).toBeDefined()
    expect(screen.queryByLabelText('warp')).toBeNull()
  })
})
