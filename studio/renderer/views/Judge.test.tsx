/**
 * @vitest-environment jsdom
 *
 * The Judge view starts only on a real set, moves a row as the player's marks
 * arrive, lists what End reported, copies the ledger, and deletes exactly what
 * was ticked (Plan 0254 Phase 3).
 */
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { LedgerLine, MarkSets, SessionInfo, SourceListing } from '@shared/judging'

import { Judge } from './Judge'

const LISTING: SourceListing = {
  dir: '/w/presets',
  families: ['attractor', 'curve'],
  proposed: false,
  files: ['attractor_clifford.toml', 'attractor_lorenz.toml', 'curve_rose.toml'],
}

const SESSION: SessionInfo = {
  run: '20261007T120000Z',
  set: 'family:attractor',
  presetDir: '/u/judging/sessions/20261007T120000Z/presets',
  presets: [
    { stem: 'attractor_clifford', name: 'Clifford' },
    { stem: 'attractor_lorenz', name: 'Lorenz' },
    { stem: 'attractor_broken', name: null },
  ],
  marks: { favourite: [], hidden: [] },
}

const line = (stem: string, verdict: LedgerLine['verdict'], run: string, source = '/w/presets'): LedgerLine => ({
  v: 1,
  run,
  set: 'family:attractor',
  source,
  stem,
  name: stem,
  verdict,
})

let api: {
  getState: ReturnType<typeof vi.fn>
  listSets: ReturnType<typeof vi.fn>
  pickSourceDir: ReturnType<typeof vi.fn>
  start: ReturnType<typeof vi.fn>
  end: ReturnType<typeof vi.fn>
  writeBack: ReturnType<typeof vi.fn>
  ledger: ReturnType<typeof vi.fn>
  listSessions: ReturnType<typeof vi.fn>
  deleteSessions: ReturnType<typeof vi.fn>
}

beforeEach(() => {
  api = {
    getState: vi.fn(() => Promise.resolve({ sourceDir: '/w/presets', session: null })),
    listSets: vi.fn(() => Promise.resolve({ ok: true, value: LISTING })),
    pickSourceDir: vi.fn(() => Promise.resolve(null)),
    start: vi.fn(() => Promise.resolve({ ok: true, value: SESSION })),
    end: vi.fn(() =>
      Promise.resolve({ ok: true, value: { run: SESSION.run, edited: ['attractor_lorenz'] } }),
    ),
    writeBack: vi.fn(() =>
      Promise.resolve({
        ok: true,
        value: {
          written: [],
          refused: [{ stem: 'attractor_lorenz', reason: '/w/presets/attractor_lorenz.toml changed since it was copied' }],
        },
      }),
    ),
    ledger: vi.fn(() => Promise.resolve([] as LedgerLine[])),
    listSessions: vi.fn(() =>
      Promise.resolve([
        { run: '20261007T120000Z', started: '2026-10-07T12:00:00Z' },
        { run: '20261006T090000Z', started: '2026-10-06T09:00:00Z' },
        { run: '20261005T080000Z', started: '2026-10-05T08:00:00Z' },
      ]),
    ),
    deleteSessions: vi.fn((runs: string[]) =>
      Promise.resolve({ ok: true, value: { deleted: runs, refused: [] } }),
    ),
  }
  Object.assign(window, { api: { judging: api } })
})

afterEach(cleanup)

async function open(marks?: MarkSets): Promise<ReturnType<typeof render>> {
  let view: ReturnType<typeof render> | undefined
  await act(async () => {
    view = render(<Judge marks={marks} onClose={vi.fn()} />)
  })
  return view as ReturnType<typeof render>
}

const startButton = (): HTMLButtonElement =>
  screen.getByRole('button', { name: 'Start' }) as HTMLButtonElement

async function click(element: HTMLElement): Promise<void> {
  await act(async () => {
    fireEvent.click(element)
  })
}

function verdictOfRow(stem: string): string | null {
  const list = screen.getByRole('list', { name: 'presets in the session' })
  const row = within(list).getByText(stem).closest('li')
  return row?.getAttribute('data-verdict') ?? null
}

describe('Start', () => {
  it('is disabled until a source directory is set', async () => {
    api.getState.mockResolvedValueOnce({ sourceDir: null, session: null })
    await open()
    expect(startButton().disabled).toBe(true)
    expect(screen.getByText(/no source directory set/)).toBeDefined()
    expect(api.listSets).not.toHaveBeenCalled()
  })

  it('is disabled while the set is empty, and enabled once it is not', async () => {
    await open()
    // A family is picked as soon as the listing names one.
    expect(startButton().disabled).toBe(false)

    await click(screen.getByLabelText('a list of files'))
    expect(startButton().disabled).toBe(true)
    await click(screen.getByLabelText('curve_rose.toml'))
    expect(startButton().disabled).toBe(false)

    await click(startButton())
    expect(api.start).toHaveBeenCalledWith({ kind: 'list', files: ['curve_rose.toml'] })
  })

  it('has nothing to start from a directory with no families', async () => {
    api.listSets.mockResolvedValueOnce({
      ok: true,
      value: { ...LISTING, families: [], files: [] },
    })
    await open()
    expect(startButton().disabled).toBe(true)
  })
})

describe('a running session', () => {
  it('moves a row from tune to keep, and to cut, as marks events arrive', async () => {
    // The marks the app held before Start are the owner's real ones, from the
    // player that was running then; they must not colour the session.
    const before: MarkSets = { favourite: ['Lorenz'], hidden: [] }
    const view = await open(before)
    await click(startButton())
    expect(api.start).toHaveBeenCalledWith({ kind: 'family', prefix: 'attractor' })
    expect(verdictOfRow('attractor_lorenz')).toBe('tune')
    expect(verdictOfRow('attractor_broken')).toBe('none')

    view.rerender(<Judge marks={{ favourite: ['Lorenz'], hidden: [] }} onClose={vi.fn()} />)
    expect(verdictOfRow('attractor_lorenz')).toBe('keep')
    expect(verdictOfRow('attractor_clifford')).toBe('tune')

    view.rerender(<Judge marks={{ favourite: [], hidden: ['Lorenz'] }} onClose={vi.fn()} />)
    expect(verdictOfRow('attractor_lorenz')).toBe('cut')
    expect(screen.getByLabelText('verdict counts').textContent).toBe('keep 0cut 1tune 1')
  })

  it('hides the Sessions list and its Delete while it runs', async () => {
    await open()
    expect(screen.getByRole('button', { name: 'Delete' })).toBeDefined()
    await click(startButton())
    expect(screen.queryByRole('button', { name: 'Delete' })).toBeNull()
    expect(screen.queryByRole('list', { name: 'session directories' })).toBeNull()
  })

  it('opens on a session already running, with the marks main folded', async () => {
    api.getState.mockResolvedValueOnce({
      sourceDir: '/w/presets',
      session: { ...SESSION, marks: { favourite: ['Clifford'], hidden: [] } },
    })
    await open({ favourite: [], hidden: [] })
    expect(verdictOfRow('attractor_clifford')).toBe('keep')
    expect(screen.queryByRole('button', { name: 'Delete' })).toBeNull()
  })
})

describe('End', () => {
  it('lists exactly the edited files the session reported, and shows a refusal by file', async () => {
    await open()
    await click(startButton())
    await click(screen.getByRole('button', { name: 'End' }))

    const edited = screen.getByRole('list', { name: 'edited files' })
    expect(within(edited).getAllByRole('listitem').map((item) => item.textContent)).toEqual([
      'attractor_lorenz',
    ])

    await click(screen.getByRole('button', { name: 'Write back' }))
    expect(api.writeBack).toHaveBeenCalledWith(['attractor_lorenz'])
    const results = screen.getByRole('list', { name: 'write-back results' })
    expect(results.textContent).toContain('attractor_lorenz')
    expect(results.textContent).toContain('changed since it was copied')
  })

  it('tells the window the session preset directory while it runs, and none after', async () => {
    const onSession = vi.fn()
    await act(async () => {
      render(<Judge marks={undefined} onClose={vi.fn()} onSession={onSession} />)
    })
    expect(onSession).toHaveBeenLastCalledWith(null)
    await click(startButton())
    expect(onSession).toHaveBeenLastCalledWith(SESSION.presetDir)
    await click(screen.getByRole('button', { name: 'End' }))
    expect(onSession).toHaveBeenLastCalledWith(null)
  })
})

describe('the ledger', () => {
  it('copies one row per stem with its latest verdict for this source directory', async () => {
    api.ledger.mockResolvedValue([
      line('attractor_lorenz', 'tune', 'r1'),
      line('attractor_clifford', 'cut', 'r1'),
      line('attractor_lorenz', 'keep', 'r2'),
      line('attractor_lorenz', 'cut', 'r3', '/elsewhere'),
    ])
    const writeText = vi.fn(() => Promise.resolve())
    Object.defineProperty(navigator, 'clipboard', { value: { writeText }, configurable: true })
    await open()

    await click(screen.getByRole('button', { name: 'Copy as Markdown' }))
    expect(writeText).toHaveBeenCalledWith(
      '| preset | verdict | run |\n' +
        '|---|---|---|\n' +
        '| attractor_clifford | cut | r1 |\n' +
        '| attractor_lorenz | keep | r2 |\n',
    )
  })
})

describe('the Sessions list', () => {
  it('sends exactly the ticked directories to Delete', async () => {
    await open()
    const list = screen.getByRole('list', { name: 'session directories' })
    const boxes = within(list).getAllByRole('checkbox')
    expect(boxes).toHaveLength(3)
    expect((screen.getByRole('button', { name: 'Delete' }) as HTMLButtonElement).disabled).toBe(true)

    await click(boxes[0])
    await click(boxes[2])
    await click(boxes[0])
    await click(boxes[1])
    await click(screen.getByRole('button', { name: 'Delete' }))
    expect(api.deleteSessions).toHaveBeenCalledTimes(1)
    expect([...(api.deleteSessions.mock.calls[0][0] as string[])].sort()).toEqual([
      '20261005T080000Z',
      '20261006T090000Z',
    ])
  })
})
