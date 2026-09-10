/**
 * @vitest-environment jsdom
 *
 * No gesture writes a preset the studio did not create (Plan 0168 Phase 1).
 *
 * The measured failure this answers is four curated presets modified in six
 * minutes from gestures their author experienced as one click, so the
 * assertions are counted writes and byte equality rather than a rendering: the
 * first gesture against a preset must write **nothing** until a name is given,
 * and what lands afterwards must land somewhere else.
 *
 * The five gestures below are the five that reach the disk — a parameter
 * release, a palette recolour, a structure key, a map row and `Ctrl+S`. They
 * are driven through the real components, because the thing being tested is
 * that no path around the gate was left open.
 */
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { EditorView } from '@codemirror/view'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { SchemaDocument } from '@shared/schema'

import { Editor, type EditorProps } from './Editor'

afterEach(cleanup)

const DIR = '/presets'
const SOURCE = '/presets/ink.toml'
const FORK = '/presets/ink_copy.toml'
const OTHER = '/presets/dust.toml'

/** A curated preset, with the shapes every gesture edits. */
const INK = [
  '# ink',
  '#',
  '# A curated preset, with a header its author wrote.',
  '',
  'name   = "ink"',
  'system = "attractor"',
  '',
  '[params]',
  'warp  = "0.4"',
  '',
  '[palette]',
  'stops = [',
  '  { at = 0.00, color = "#101014" },  # ground',
  '  { at = 1.00, color = "#6ee7d0" },  # crest',
  ']',
  '',
  '[hold]',
  'drift = "beat"',
  '',
].join('\n')

const DUST = ['name   = "dust"', 'system = "attractor"', '', '[params]', 'warp  = "0.1"', ''].join(
  '\n',
)

const SCHEMA: SchemaDocument = {
  v: 1,
  hash: 'test',
  systems: [
    {
      name: 'attractor',
      params: [
        { name: 'warp', default: 0.4, range: [0, 1.5], doc: 'Amplitude.', kind: 'modal' },
        { name: 'drift', default: 1, range: [0, 4], doc: 'How fast.', kind: 'modal' },
      ],
    },
    { name: 'swarm', params: [] },
  ],
  stages: [],
  tables: [
    {
      name: 'preset',
      doc: 'The document itself.',
      keys: [
        { name: 'system', default: '', doc: 'Which system.', kind: 'string' },
        { name: 'hold', default: '', doc: 'Sample and hold.', kind: 'map', of: { kind: 'hold' } },
      ],
    },
    {
      name: 'palette',
      doc: 'Colour.',
      keys: [
        { name: 'name', default: '', doc: 'A built-in.', kind: 'enum', values: ['ember', 'ice'] },
      ],
    },
  ],
  grammar: { variables: ['bass'], functions: [], constants: [] },
}

interface Write {
  path: string
  text: string
  how: 'write' | 'create'
}

interface Fake {
  files: Map<string, string>
  writes: Write[]
}

/** A filesystem the writes land in, so byte equality can be asserted on it. */
function install(seed: Record<string, string> = { [SOURCE]: INK }): Fake {
  const fake: Fake = { files: new Map(Object.entries(seed)), writes: [] }
  const api = {
    app: {
      getSchema: () => Promise.resolve({ ok: true as const, document: SCHEMA }),
    },
    player: { send: vi.fn() },
    preset: {
      read: (path: string) => {
        const text = fake.files.get(path)
        return Promise.resolve(
          text === undefined
            ? { ok: false as const, reason: 'no such file' }
            : { ok: true as const, value: { path, text } },
        )
      },
      write: (path: string, text: string) => {
        fake.writes.push({ path, text, how: 'write' })
        fake.files.set(path, text)
        return Promise.resolve({ ok: true as const, value: null })
      },
      create: (path: string, text: string) => {
        if (fake.files.has(path)) {
          return Promise.resolve({ ok: false as const, reason: 'already there' })
        }
        fake.writes.push({ path, text, how: 'create' })
        fake.files.set(path, text)
        return Promise.resolve({ ok: true as const, value: null })
      },
    },
  }
  window.api = api as unknown as typeof window.api
  return fake
}

function editor(overrides: Partial<EditorProps> = {}) {
  const onProblem = vi.fn()
  const props: EditorProps = {
    system: 'attractor',
    file: SOURCE,
    reloads: 0,
    problems: [],
    roster: ['ink'],
    active: 'ink',
    dir: DIR,
    onProblem,
    ...overrides,
  }
  const view = render(<Editor {...props} />)
  return {
    onProblem,
    rerender: (next: Partial<EditorProps>) => view.rerender(<Editor {...props} {...next} />),
  }
}

async function tab(name: string): Promise<void> {
  fireEvent.click(await screen.findByRole('tab', { name }))
}

/** Type a name into the held prompt and save it. */
async function saveAs(name: string): Promise<void> {
  const field = await screen.findByLabelText('save a copy as')
  fireEvent.change(field, { target: { value: name } })
  fireEvent.click(screen.getByRole('button', { name: 'save' }))
  await waitFor(() => expect(screen.queryByLabelText('save a copy as')).toBeNull())
}

/** The five gestures that reach the disk, and the mark each leaves. */
const GESTURES: { name: string; run: () => Promise<void>; mark: string }[] = [
  {
    name: 'a parameter release',
    mark: 'warp  = "0.9"',
    run: async () => {
      const slider = await screen.findByLabelText('warp')
      fireEvent.change(slider, { target: { value: '0.9' } })
      fireEvent.pointerUp(slider)
    },
  },
  {
    name: 'a palette recolour',
    mark: '#ff0000',
    run: async () => {
      await tab('palette')
      fireEvent.change(screen.getByLabelText('stop 1 colour'), { target: { value: '#ff0000' } })
    },
  },
  {
    name: 'a structure key',
    mark: 'system = "swarm"',
    run: async () => {
      await tab('structure')
      fireEvent.change(screen.getByLabelText('system'), { target: { value: 'swarm' } })
      fireEvent.click(screen.getByRole('button', { name: 'change to swarm' }))
    },
  },
  {
    name: 'a map row',
    mark: 'drift = "bar"',
    run: async () => {
      await tab('structure')
      fireEvent.blur(screen.getByLabelText('drift'), { target: { value: 'bar' } })
    },
  },
  {
    name: 'Ctrl+S in the file editor',
    mark: '# edited by hand',
    run: async () => {
      await tab('file')
      const host = await screen.findByLabelText('preset file')
      const view = EditorView.findFromDOM(host as HTMLElement)
      if (view === null) throw new Error('the file tab has no editor view')
      await waitFor(() => expect(view.state.doc.length).toBeGreaterThan(0))
      // The dispatch runs CodeMirror's update listener, which sets React state
      // on the component around it; without `act` that lands outside the render
      // pass and React says so.
      act(() => view.dispatch({ changes: { from: 0, insert: '# edited by hand\n' } }))
      fireEvent.keyDown(view.contentDOM, { key: 's', ctrlKey: true })
    },
  },
]

describe('a gesture against a preset the session has not forked', () => {
  it.each(GESTURES)('$name writes nothing until the copy is named', async ({ run, mark }) => {
    const fake = install()
    editor()
    await run()

    // The whole assertion the smoke run's four modified files exist for.
    await screen.findByLabelText('save a copy as')
    expect(fake.writes).toHaveLength(0)

    await saveAs('ink copy')
    expect(fake.writes).toHaveLength(1)
    expect(fake.writes[0]).toMatchObject({ path: FORK, how: 'create' })
    expect(fake.writes[0].text).toContain(mark)
    // The fork carries its own name, so the roster holds no two presets under
    // one name.
    expect(fake.writes[0].text).toContain('name   = "ink copy"')
    expect(fake.files.get(SOURCE)).toBe(INK)
  })
})

describe('the preset that was forked from', () => {
  it('is byte-identical after a whole editing session against the fork', async () => {
    const fake = install()
    const { rerender } = editor()

    const slider = await screen.findByLabelText('warp')
    fireEvent.change(slider, { target: { value: '0.9' } })
    fireEvent.pointerUp(slider)
    await saveAs('ink copy')

    // The player picks the new file up and reports it, which is when the editor
    // is on the fork.
    rerender({ file: FORK, active: 'ink copy', roster: ['ink', 'ink copy'] })
    await screen.findByText(FORK)

    for (const value of ['0.8', '0.7', '0.6']) {
      const handle = await screen.findByLabelText('warp')
      fireEvent.change(handle, { target: { value } })
      fireEvent.pointerUp(handle)
      await waitFor(() => expect(fake.files.get(FORK)).toContain(`"${value}"`))
    }

    // Not a line count: equality.
    expect(fake.files.get(SOURCE)).toBe(INK)
    expect(fake.writes.filter((write) => write.path === SOURCE)).toHaveLength(0)
    // Three silent writes after the one create: the live loop is intact.
    expect(fake.writes.map((write) => write.how)).toEqual(['create', 'write', 'write', 'write'])
  })
})

describe('a preset change between the gesture and the answer', () => {
  it('cannot redirect the write', async () => {
    const fake = install({ [SOURCE]: INK, [OTHER]: DUST })
    const { rerender } = editor()

    const slider = await screen.findByLabelText('warp')
    fireEvent.change(slider, { target: { value: '0.9' } })
    fireEvent.pointerUp(slider)
    await screen.findByLabelText('save a copy as')

    // Rotation, or a click in the library: the preset under the editor changes
    // while the prompt is up.
    rerender({ file: OTHER, active: 'dust', roster: ['ink', 'dust'] })
    await saveAs('ink copy')

    expect(fake.writes).toHaveLength(1)
    expect(fake.writes[0].path).toBe(FORK)
    // The document the gesture was made against, not the one that arrived after.
    expect(fake.writes[0].text).toContain('warp  = "0.9"')
    expect(fake.writes[0].text).toContain('# A curated preset, with a header its author wrote.')
    expect(fake.files.get(SOURCE)).toBe(INK)
    expect(fake.files.get(OTHER)).toBe(DUST)
  })
})

describe('a preset from the embedded set', () => {
  it('is editable, and its fork is its first file', async () => {
    const fake = install({})
    editor({ file: null, active: 'ember', roster: ['ember'] })

    const slider = await screen.findByLabelText('warp')
    fireEvent.change(slider, { target: { value: '0.9' } })
    fireEvent.pointerUp(slider)
    await screen.findByLabelText('save a copy as')
    expect(fake.writes).toHaveLength(0)

    await saveAs('mine')
    expect(fake.writes).toHaveLength(1)
    expect(fake.writes[0]).toMatchObject({ path: '/presets/mine.toml', how: 'create' })
    expect(fake.writes[0].text).toContain('system = "attractor"')
    expect(fake.writes[0].text).toContain('warp = "0.9"')
  })
})

describe('the prompt itself', () => {
  it('writes nothing when it is cancelled, and drops the edits it held', async () => {
    const fake = install()
    editor()

    const slider = await screen.findByLabelText('warp')
    fireEvent.change(slider, { target: { value: '0.9' } })
    fireEvent.pointerUp(slider)
    fireEvent.click(await screen.findByRole('button', { name: 'cancel' }))

    await screen.findByText(SOURCE)
    expect(fake.writes).toHaveLength(0)
    expect(fake.files.get(SOURCE)).toBe(INK)
  })

  it('stays up when the name is already taken', async () => {
    const fake = install({ [SOURCE]: INK, [FORK]: DUST })
    const { onProblem } = editor()

    const slider = await screen.findByLabelText('warp')
    fireEvent.change(slider, { target: { value: '0.9' } })
    fireEvent.pointerUp(slider)

    const field = await screen.findByLabelText('save a copy as')
    fireEvent.change(field, { target: { value: 'ink copy' } })
    fireEvent.click(screen.getByRole('button', { name: 'save' }))

    await waitFor(() => expect(onProblem).toHaveBeenCalledWith('already there'))
    expect(screen.getByLabelText('save a copy as')).toBeDefined()
    expect(fake.writes).toHaveLength(0)
    expect(fake.files.get(FORK)).toBe(DUST)
  })
})
