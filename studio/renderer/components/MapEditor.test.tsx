/**
 * @vitest-environment jsdom
 *
 * `[hold]` has an editor, and it is the schema that gives it one
 * (Plan 0167 Phase 6).
 *
 * The rows are the file's — a map's keys are author-chosen parameter names —
 * and the control is the schema's, resolved from `of.kind`. Nothing about hold
 * is written into the component, which is why the same component renders
 * `[smoothing]`.
 */
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { TableKey } from '@shared/schema'
import type { Key } from '@shared/toml'

import { MapEditor } from './MapEditor'

afterEach(cleanup)

const HOLD: TableKey = {
  name: 'hold',
  default: '',
  doc: 'Per-parameter sample-and-hold; an unlisted parameter is read every frame.',
  kind: 'map',
  of: { kind: 'hold' },
}

const ENTRIES: Key[] = [
  { name: 'petals', literal: '"beat"', line: 8 },
  { name: 'warp', literal: '0.5', line: 9 },
]

function editor(overrides: Partial<Parameters<typeof MapEditor>[0]> = {}) {
  const onSet = vi.fn()
  const onRemove = vi.fn()
  render(
    <MapEditor
      section="hold"
      spec={HOLD}
      control="scalar"
      entries={ENTRIES}
      suggestions={['petals', 'warp', 'drift']}
      writable
      onSet={onSet}
      onRemove={onRemove}
      {...overrides}
    />,
  )
  return { onSet, onRemove }
}

describe('the entries a preset holds', () => {
  it('renders one row per entry in the file, and none of its own', () => {
    editor()
    expect(screen.getByLabelText('petals')).toHaveProperty('value', 'beat')
    expect(screen.getByLabelText('warp')).toHaveProperty('value', '0.5')
    expect(screen.queryByLabelText('drift')).toBeNull()
  })

  it('says so rather than showing an empty box when the preset holds nothing', () => {
    editor({ entries: [] })
    expect(screen.getByText('nothing held')).toBeDefined()
  })

  it('writes a word as a string and a number as a number', () => {
    const { onSet } = editor()
    fireEvent.blur(screen.getByLabelText('petals'), { target: { value: 'bar' } })
    expect(onSet).toHaveBeenCalledWith('hold', 'petals', '"bar"')
    fireEvent.blur(screen.getByLabelText('warp'), { target: { value: '2' } })
    expect(onSet).toHaveBeenCalledWith('hold', 'warp', '2')
  })

  it('removes an entry by name', () => {
    const { onRemove } = editor()
    fireEvent.click(screen.getByLabelText('remove petals'))
    expect(onRemove).toHaveBeenCalledWith('hold', 'petals')
  })
})

describe('adding an entry', () => {
  it('writes the name and the value together, and clears the row', () => {
    const { onSet } = editor()
    const name = screen.getByLabelText('add to hold') as HTMLInputElement
    const value = screen.getByLabelText('value for the new hold entry') as HTMLInputElement
    fireEvent.change(name, { target: { value: 'drift' } })
    fireEvent.change(value, { target: { value: 'bar' } })
    fireEvent.click(screen.getByText('add'))

    expect(onSet).toHaveBeenCalledWith('hold', 'drift', '"bar"')
    expect(name.value).toBe('')
    expect(value.value).toBe('')
  })

  it('writes nothing for a half-filled row', () => {
    const { onSet } = editor()
    fireEvent.change(screen.getByLabelText('add to hold'), { target: { value: 'drift' } })
    fireEvent.click(screen.getByText('add'))
    expect(onSet).not.toHaveBeenCalled()
  })

  it('offers the parameters the preset may bind, as suggestions and not a closed list', () => {
    editor()
    const list = document.getElementById('map-hold-names')
    const options = [...(list?.children ?? [])].map((option) => (option as HTMLOptionElement).value)
    expect(options).toEqual(['petals', 'warp', 'drift'])
    // Free text, because a name the system does not declare is the engine's
    // refusal to report, not the studio's to pre-empt.
    expect((screen.getByLabelText('add to hold') as HTMLInputElement).tagName).toBe('INPUT')
  })

  it('offers the two edges a hold accepts beside a number of seconds', () => {
    editor()
    const edges = document.getElementById('map-hold-edges')
    const options = [...(edges?.children ?? [])].map((o) => (o as HTMLOptionElement).value)
    expect(options).toEqual(['beat', 'bar'])
  })

  it('offers no edges for a map of some other kind, because only a hold has them', () => {
    editor({
      spec: { name: 'smoothing', default: '', doc: 'Easing.', kind: 'map', of: { kind: 'easing' } },
      section: 'smoothing',
      entries: [],
    })
    expect(document.getElementById('map-smoothing-edges')).toBeNull()
  })
})

describe('a preset with no file behind it', () => {
  it('offers no writes, because there is nowhere for them to land', () => {
    editor({ writable: false })
    expect((screen.getByLabelText('petals') as HTMLInputElement).disabled).toBe(true)
    expect((screen.getByText('add') as HTMLButtonElement).disabled).toBe(true)
  })
})
