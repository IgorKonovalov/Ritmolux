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

import { rostersFor, schemaDocumentSchema, type ParamRoster } from '@shared/schema'
import type { Binding } from '@shared/toml'

// The engine's own committed document, imported rather than read: the renderer
// never imports Node, and that rule holds for a renderer test too.
import PLAYER_SCHEMA from '../../../docs/specs/player-schema.json'

import { ParamPanel } from './ParamPanel'

afterEach(cleanup)

const ROSTERS: ParamRoster[] = [
  {
    name: 'fragment_field',
    params: [
      { name: 'warp', default: 0.4, range: [0, 1.5], doc: 'Amplitude of the fold.', kind: 'modal' },
      { name: 'drift', default: 1, range: [0, 4], doc: 'How fast it turns.', kind: 'modal' },
      {
        name: 'mirror_order',
        default: 6,
        range: [0, 12],
        doc: 'How many mirrors.',
        kind: 'structural',
      },
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
      family={null}
      bindings={BINDINGS}
      writable
      hasDocument
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

  it('is inert while the preset document is still unknown', () => {
    const { onDrag, onCommit } = panel({ hasDocument: false })
    const slider = screen.getByLabelText('warp')

    // Not the same state as the row above. There the preset is known and has
    // nowhere to be written; here it is not known at all, so the row is showing
    // the engine default rather than this preset's value. Dragging would report
    // a number the preset does not hold, and the release would be discarded.
    expect((slider as HTMLInputElement).disabled).toBe(true)

    fireEvent.change(slider, { target: { value: '1.1' } })
    fireEvent.pointerUp(slider)
    expect(onDrag).not.toHaveBeenCalled()
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
      rosters: [
        {
          name: 'invented',
          params: [{ name: 'x', default: 0, range: [0, 1], doc: '', kind: 'modal' }],
        },
      ],
      bindings: [],
    })
    expect(screen.getByLabelText('x')).toBeDefined()
    expect(screen.queryByLabelText('warp')).toBeNull()
  })
})

/**
 * A parameter the engine rounds is not offered a continuous control
 * (Plan 0167 Phase 6).
 *
 * `kind` is the whole of the rule: `structural` means the engine quantizes the
 * value once before the scene sees it (ADR-0180 rule 2), so a slider offering
 * three quarters of a step offers travel that draws the same picture — and a
 * value sent unrounded would have the studio reporting 6.4 mirrors while the
 * player drew 6.
 */
describe('what the kind decides', () => {
  it('gives a structural parameter whole steps', () => {
    panel()
    expect(screen.getByLabelText('mirror_order')).toHaveProperty('step', '1')
  })

  it('leaves a modal parameter a step fine enough to read as continuous', () => {
    panel()
    const slider = screen.getByLabelText('warp') as HTMLInputElement
    expect(Number(slider.step)).toBeLessThan(0.01)
  })

  it('sends the value the engine would have rounded to, not the one dragged', () => {
    const { onDrag, onCommit } = panel({ bindings: [] })
    const slider = screen.getByLabelText('mirror_order')
    fireEvent.change(slider, { target: { value: '6.4' } })
    expect(onDrag).toHaveBeenCalledWith('mirror_order', 6)
    fireEvent.pointerUp(slider)
    expect(onCommit).toHaveBeenCalledWith('mirror_order', 6)
  })

  it('reads out a whole number rather than three decimals of one', () => {
    panel({ bindings: [] })
    expect(screen.getByText('6')).toBeDefined()
  })

  it('rounds a fractional value the file holds, because the engine will', () => {
    panel({
      bindings: [{ kind: 'const', name: 'mirror_order', value: 5.7, line: 7, quote: '"' }],
    })
    expect(screen.getByLabelText('mirror_order')).toHaveProperty('value', '6')
  })
})

/**
 * A slider's ends come from the family on screen (Plan 0179 Phase 5).
 *
 * Against the **engine's own committed document**, not a fixture: the ranges
 * asserted below are declarations `FAMILY_PARAMS` makes, and a hand-written copy
 * of them here would pass while the panel showed something else. The join is the
 * point of ADR-0194 — `families` is static and travels in the schema, `family`
 * is a fact about what the player loaded and arrives on the `preset` event, and
 * a wrong slider is what either one alone produces.
 */
describe('what the family on screen decides', () => {
  const DOCUMENT = schemaDocumentSchema.parse(PLAYER_SCHEMA)

  function real(system: string, family: string | null, overrides = {}) {
    return panel({ rosters: rostersFor(DOCUMENT, system), family, bindings: [], ...overrides })
  }

  /** The names in the trailing group, which the heading names the family of. */
  function notRead(family: string): string[] {
    const heading = screen.getByRole('heading', { name: `not read on ${family}` })
    const group = heading.parentElement
    if (group === null) throw new Error('the inert heading has no group')
    return [...group.querySelectorAll('span')]
      .map((span) => span.textContent ?? '')
      .filter((text) => text !== '' && text !== 'not read here')
  }

  it('reaches the negative half of a hypotrochoid, which the single range cannot', () => {
    real('parametric_curve', 'hypotrochoid')
    const n = screen.getByLabelText('n') as HTMLInputElement
    expect(n.min).toBe('-8')
    expect(n.max).toBe('8')
  })

  it('stops a Lissajous d at the twelve it reads, not the three hundred it does not', () => {
    real('parametric_curve', 'lissajous')
    expect((screen.getByLabelText('d') as HTMLInputElement).max).toBe('12')
  })

  it('groups a parameter the family never reads instead of offering it travel', () => {
    real('parametric_curve', 'superformula')
    expect(notRead('superformula')).toContain('n')
    expect(screen.queryByLabelText('n')).toBeNull()
  })

  it('still shows the binding of a row the family does not read', () => {
    // Not hidden, because the file may carry one: a preset written for another
    // family keeps its value, and switching back makes it live again.
    real('parametric_curve', 'superformula', {
      bindings: [{ kind: 'const', name: 'n', value: 5, line: 4, quote: '"' }],
    })
    expect(notRead('superformula')).toContain('5')
  })

  it('gives the attractor coefficients Thomas reads a slider and groups the other three', () => {
    // The clearest case in the engine: all four declare `range: null`, so before
    // the family was known every one of them was a bare number field whatever
    // was drawn. Thomas reads `a` alone.
    real('attractor', 'thomas')
    const a = screen.getByLabelText('a') as HTMLInputElement
    expect(a.type).toBe('range')
    expect([a.min, a.max]).toEqual(['0', '0.25'])
    expect(notRead('thomas')).toEqual(expect.arrayContaining(['b', 'c', 'd']))
  })

  it('gives all four a slider on a family that reads all four', () => {
    real('attractor', 'de_jong')
    for (const name of ['a', 'b', 'c', 'd']) {
      const input = screen.getByLabelText(name) as HTMLInputElement
      expect([name, input.type, input.min, input.max]).toEqual([name, 'range', '-3', '3'])
    }
    expect(screen.queryByRole('heading', { name: /^not read on / })).toBeNull()
  })

  it('falls back to the single range when the player reports no family', () => {
    real('parametric_curve', null)
    const n = screen.getByLabelText('n') as HTMLInputElement
    expect([n.min, n.max]).toEqual(['1', '24'])
    expect(screen.queryByRole('heading', { name: /^not read on / })).toBeNull()
  })

  it('falls back to the single range when the document declares no families', () => {
    // A player older than ADR-0194 prints `range` alone. Naming a family it never
    // reported must not empty the panel or invent an end.
    const stripped: ParamRoster[] = rostersFor(DOCUMENT, 'parametric_curve').map((roster) => ({
      ...roster,
      params: roster.params.map((spec) => ({ ...spec, families: undefined })),
    }))
    panel({ rosters: stripped, family: 'hypotrochoid', bindings: [] })
    const n = screen.getByLabelText('n') as HTMLInputElement
    expect([n.min, n.max]).toEqual(['1', '24'])
    expect(screen.queryByRole('heading', { name: /^not read on / })).toBeNull()
  })
})
