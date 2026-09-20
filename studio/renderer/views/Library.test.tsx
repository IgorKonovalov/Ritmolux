/**
 * @vitest-environment jsdom
 *
 * The marks are the player's, and the list renders what it was told
 * (Plan 0205 Phase 6).
 *
 * Three claims, and they are the ones ADR-0229 rests on: a toggle sends the
 * *state* it wants rather than a press, nothing moves in the list until the
 * player reports the new sets, and a studio that has heard no `marks` shows no
 * marks at all — which is a different picture from a library with nothing
 * marked.
 */
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { SchemaDocument } from '@shared/schema'

import { Library, type LibraryProps } from './Library'

afterEach(cleanup)

const SCHEMA: SchemaDocument = {
  v: 1,
  hash: 'test',
  systems: [{ name: 'attractor', params: [] }],
  stages: [],
  tables: [],
  grammar: { variables: [], functions: [], constants: [] },
}

const ROSTER = ['Aurora', 'Clifford', 'Ember']

function library(overrides: Partial<LibraryProps> = {}) {
  const onSelect = vi.fn()
  const onMark = vi.fn()
  const onCreate = vi.fn()
  const props: LibraryProps = {
    document: SCHEMA,
    roster: ROSTER,
    active: 'Aurora',
    dir: '/presets',
    system: 'attractor',
    marks: { favourite: ['Clifford'], hidden: [] },
    onSelect,
    onMark,
    onCreate,
    onProblem: vi.fn(),
    ...overrides,
  }
  const view = render(<Library {...props} />)
  /** What a fresh `marks` event does to the mounted list. */
  const report = (marks: LibraryProps['marks']): void => {
    view.rerender(<Library {...props} marks={marks} />)
  }
  return { onSelect, onMark, onCreate, report }
}

/** The names the list is showing, in the order it shows them. */
function listed(): string[] {
  return screen
    .getAllByRole('button')
    .filter((button) => button.getAttribute('aria-current') !== null)
    .map((button) => button.textContent ?? '')
}

describe('a mark the player reported', () => {
  it('lights the row it names, and no other', () => {
    library()
    expect(screen.getByLabelText('favourite Clifford')).toHaveProperty('ariaPressed', 'true')
    expect(screen.getByLabelText('favourite Aurora')).toHaveProperty('ariaPressed', 'false')
  })

  it('follows a change made at the player keyboard, with no restart', () => {
    // Nothing was clicked here: this is the unsolicited `marks` line ADR-0229
    // exists to deliver, and the list has to move on it.
    const { report } = library({ marks: { favourite: [], hidden: [] } })
    report({ favourite: ['Aurora'], hidden: ['Ember'] })
    expect(screen.getByLabelText('favourite Aurora')).toHaveProperty('ariaPressed', 'true')
    expect(screen.getByLabelText('hide Ember')).toHaveProperty('ariaPressed', 'true')
  })
})

describe('a mark made in the studio', () => {
  it('asks for the state it wants rather than for a toggle', () => {
    const { onMark } = library()
    fireEvent.click(screen.getByLabelText('favourite Aurora'))
    expect(onMark).toHaveBeenCalledWith('Aurora', 'favourite', true)
    fireEvent.click(screen.getByLabelText('favourite Clifford'))
    expect(onMark).toHaveBeenCalledWith('Clifford', 'favourite', false)
  })

  it('sends hidden under its own word', () => {
    const { onMark } = library()
    fireEvent.click(screen.getByLabelText('hide Ember'))
    expect(onMark).toHaveBeenCalledWith('Ember', 'hidden', true)
  })

  it('moves nothing in the list until the player says so', () => {
    // The studio writes no marks file (ADR-0229), so the row it just clicked is
    // still unmarked: what lights it is the `marks` event coming back.
    const { onMark } = library()
    fireEvent.click(screen.getByLabelText('favourite Aurora'))
    expect(onMark).toHaveBeenCalledTimes(1)
    expect(screen.getByLabelText('favourite Aurora')).toHaveProperty('ariaPressed', 'false')
  })
})

describe('narrowing to favourites', () => {
  it('shows only the favourites while it is on, and the whole roster after', () => {
    library()
    const filter = screen.getByLabelText(/favourites only/)
    fireEvent.click(filter)
    expect(listed()).toEqual(['Clifford'])
    fireEvent.click(filter)
    expect(listed()).toEqual(ROSTER)
  })

  it('says so rather than looking like a lost roster when nothing is favourite', () => {
    library({ marks: { favourite: [], hidden: [] } })
    fireEvent.click(screen.getByLabelText(/favourites only/))
    expect(listed()).toEqual([])
    expect(screen.getByText('no preset is marked favourite')).toBeTruthy()
  })

  it('keeps a hidden preset listed, because this is where it is unhidden', () => {
    library({ marks: { favourite: [], hidden: ['Ember'] } })
    expect(listed()).toEqual(ROSTER)
    expect(screen.getByLabelText('hide Ember')).toHaveProperty('ariaPressed', 'true')
  })
})

describe('a studio no player has reported marks to', () => {
  it('shows no marks rather than an empty set, and offers no filter', () => {
    library({ marks: undefined })
    expect(screen.queryByLabelText('favourite Aurora')).toBeNull()
    expect(screen.queryByLabelText(/favourites only/)).toBeNull()
    expect(screen.getByText(/No player has reported its marks/)).toBeTruthy()
    // The roster itself is still the player's and is still listed.
    expect(listed()).toEqual(ROSTER)
  })
})
