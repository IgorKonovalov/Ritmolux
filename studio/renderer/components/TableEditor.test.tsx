/**
 * @vitest-environment jsdom
 *
 * The table renders the keys the schema declares, and writes the literal the
 * kind decides (Plan 0159 Phase 8) — and a map of a kind that has a control
 * renders as rows rather than as a read-only word (Plan 0167 Phase 6).
 *
 * The walk over every kind the engine declares lives in `shared/fields.test.ts`,
 * which may read the built player's document; this file may not, because a
 * renderer file has no Node.
 */
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { TableSpec } from '@shared/schema'
import type { Key } from '@shared/toml'

import { TableEditor } from './TableEditor'

afterEach(cleanup)

const table: TableSpec = {
  name: 'feedback',
  doc: 'The feedback table.',
  keys: [
    { name: 'warp', default: 'none', doc: 'Which warp.', kind: 'enum', values: ['none', 'zoom'] },
    { name: 'deposit', default: 'add', doc: 'How light lands.', kind: 'text' },
  ],
}

function editor(overrides: Partial<Parameters<typeof TableEditor>[0]> = {}) {
  const onSet = vi.fn()
  const onRemove = vi.fn()
  render(
    <TableEditor
      table={table}
      keys={[]}
      readSection={() => []}
      suggestions={[]}
      writable
      onSet={onSet}
      onRemove={onRemove}
      {...overrides}
    />,
  )
  return { onSet, onRemove }
}

describe('a rendered table', () => {
  it('renders one control per key the schema declares, and no others', () => {
    editor()
    for (const key of table.keys) {
      expect(screen.getByText(key.name), `no row for \`${key.name}\``).toBeDefined()
    }
  })

  it('marks a key the preset does not hold as sitting at its default', () => {
    editor()
    expect(screen.getAllByText('default').length).toBe(table.keys.length)
  })

  it('writes the literal the kind decides, not the string the control held', () => {
    const { onSet } = editor()
    const enumKey = table.keys.find((key) => key.kind === 'enum')
    if (enumKey === undefined) return
    const select = screen.getByLabelText(enumKey.name) as HTMLSelectElement
    const option = (enumKey.values ?? [])[1]
    if (option === undefined) return
    select.value = option
    select.dispatchEvent(new Event('change', { bubbles: true }))
    expect(onSet).toHaveBeenCalledWith(table.name, enumKey.name, `"${option}"`)
  })
})

/**
 * A map's entries are the author's own, so they come from the file; what one
 * entry accepts is declared, so the control comes from the schema. Neither half
 * is listed in this component.
 */
describe('a map key of a table', () => {
  const withHold: TableSpec = {
    name: 'layer',
    doc: 'A layer.',
    keys: [
      { name: 'blend', default: 'add', doc: 'How it lands.', kind: 'text' },
      {
        name: 'hold',
        default: '',
        doc: 'Per-parameter sample-and-hold.',
        kind: 'map',
        of: { kind: 'hold' },
      },
    ],
  }
  const held: Key[] = [{ name: 'petals', literal: '"beat"', line: 4 }]
  const readHold = (section: string): Key[] => (section === 'layer.hold' ? held : [])

  it('renders the map as its own nested section, not as a value on one line', () => {
    editor({ table: withHold, readSection: readHold })
    expect(screen.getByText('[layer.hold]')).toBeDefined()
    expect(screen.getByLabelText('petals')).toHaveProperty('value', 'beat')
  })

  it('writes an edited entry into the nested section', () => {
    const { onSet } = editor({ table: withHold, readSection: readHold })
    fireEvent.blur(screen.getByLabelText('petals'), { target: { value: 'bar' } })
    expect(onSet).toHaveBeenCalledWith('layer.hold', 'petals', '"bar"')
  })

  it('leaves a map whose entries are expressions to the panel and the file tab', () => {
    const withParams: TableSpec = {
      name: 'layer',
      doc: 'A layer.',
      keys: [{ name: 'params', default: '', doc: 'Bindings.', kind: 'map', of: { kind: 'expr' } }],
    }
    editor({ table: withParams })
    // Shown, and shown as unedited: a third control for an expression would
    // compete with the slider and with the file editor.
    expect(screen.queryByText('[layer.params]')).toBeNull()
    expect(screen.getByText('(map)')).toBeDefined()
  })
})
