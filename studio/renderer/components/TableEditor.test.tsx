/**
 * @vitest-environment jsdom
 *
 * The table renders the keys the schema declares, and writes the literal the
 * kind decides (Plan 0159 Phase 8).
 *
 * The walk over every kind the engine declares lives in `shared/fields.test.ts`,
 * which may read the built player's document; this file may not, because a
 * renderer file has no Node.
 */
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { TableSpec } from '@shared/schema'

import { TableEditor } from './TableEditor'

afterEach(cleanup)

describe('a rendered table', () => {
  const table: TableSpec = {
    name: 'feedback',
    doc: 'The feedback table.',
    keys: [
      { name: 'warp', default: 'none', doc: 'Which warp.', kind: 'enum', values: ['none', 'zoom'] },
      { name: 'deposit', default: 'add', doc: 'How light lands.', kind: 'text' },
    ],
  }

  it('renders one control per key the schema declares, and no others', () => {
    render(<TableEditor table={table} keys={[]} writable onSet={vi.fn()} />)
    for (const key of table.keys) {
      expect(screen.getByText(key.name), `no row for \`${key.name}\``).toBeDefined()
    }
  })

  it('marks a key the preset does not hold as sitting at its default', () => {
    render(<TableEditor table={table} keys={[]} writable onSet={vi.fn()} />)
    expect(screen.getAllByText('default').length).toBe(table.keys.length)
  })

  it('writes the literal the kind decides, not the string the control held', () => {
    const onSet = vi.fn()
    const enumKey = table.keys.find((key) => key.kind === 'enum')
    if (enumKey === undefined) return
    render(<TableEditor table={table} keys={[]} writable onSet={onSet} />)
    const select = screen.getByLabelText(enumKey.name) as HTMLSelectElement
    const option = (enumKey.values ?? [])[1]
    if (option === undefined) return
    select.value = option
    select.dispatchEvent(new Event('change', { bubbles: true }))
    expect(onSet).toHaveBeenCalledWith(table.name, enumKey.name, `"${option}"`)
  })
})
