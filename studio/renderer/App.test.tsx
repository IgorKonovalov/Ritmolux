/**
 * @vitest-environment jsdom
 *
 * Every problem is reachable, not only the first (Plan 0168 Phase 2).
 *
 * Driven through the real event reducer rather than through a list of props:
 * how many problems are kept, in what order, and what happens past the bound
 * are the reducer's answers, and a test that handed the modal its own array
 * would assert the modal's opinion of them instead.
 */
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import type { PlayerEvent } from '@shared/protocol'

import { App } from './App'

/** The event listener the application registered, so a test can push events. */
let push: (event: PlayerEvent) => void

beforeEach(() => {
  vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
    putImageData: () => undefined,
  } as unknown as CanvasRenderingContext2D)

  const api = {
    app: {
      getInfo: () =>
        Promise.resolve({
          studioVersion: '0.115.0',
          playerPath: '/bin/ritmolux',
          playerSource: 'PATH',
          playerMode: 'windowed',
        }),
      // Refused on purpose: the panel is not what is under test, and without a
      // schema it renders one line instead of every row the engine declares.
      getSchema: () => Promise.resolve({ ok: false as const, reason: 'no player in this test' }),
    },
    player: {
      send: vi.fn(),
      onEvent: (listener: (event: PlayerEvent) => void) => {
        push = listener
        return () => undefined
      },
    },
    preset: { read: vi.fn(), write: vi.fn(), create: vi.fn() },
  }
  window.api = api as unknown as typeof window.api
})

afterEach(() => {
  cleanup()
  vi.restoreAllMocks()
})

const error = (file: string, message: string, line: number | null = 3): PlayerEvent =>
  ({ v: 1, ev: 'preset_error', file, message, line, col: 1, param: null }) as PlayerEvent

const warning = (file: string, message: string): PlayerEvent =>
  ({ v: 1, ev: 'preset_warning', file, message }) as PlayerEvent

function report(...events: PlayerEvent[]): void {
  act(() => {
    for (const event of events) push(event)
  })
}

/**
 * Mount the window and let its two startup fetches land.
 *
 * `getInfo` and `getSchema` are promises the application starts on mount, so a
 * synchronous render leaves two state updates arriving outside the render pass.
 */
async function open(): Promise<void> {
  await act(async () => {
    render(<App />)
  })
}

/** Open the list from the banner's count. */
function openList(): HTMLElement {
  fireEvent.click(screen.getByRole('button', { name: /problems$/ }))
  return screen.getByRole('dialog', { name: 'problems' })
}

describe('the banner', () => {
  it('shows the newest problem alone when it is the only one', async () => {
    await open()
    report(error('/presets/ink.toml', 'unexpected token'))
    expect(screen.getByText(/unexpected token/)).toBeDefined()
    expect(screen.queryByRole('button', { name: /problems$/ })).toBeNull()
  })

  it('counts them, and the list holds every one', async () => {
    await open()
    report(
      error('/presets/a.toml', 'first thing'),
      warning('/presets/b.toml', 'second thing'),
      error('/presets/c.toml', 'third thing'),
    )

    expect(screen.getByRole('button', { name: '3 problems' })).toBeDefined()
    const items = within(openList()).getAllByRole('listitem')
    // Newest first, each carrying its own file and its own message: the
    // assertion that says this is the reducer's list and not its head repeated.
    expect(items.map((item) => item.textContent)).toEqual([
      expect.stringContaining('third thing'),
      expect.stringContaining('second thing'),
      expect.stringContaining('first thing'),
    ])
    expect(items[0].textContent).toContain('/presets/c.toml')
    expect(items[1].textContent).toContain('/presets/b.toml')
    expect(items[2].textContent).toContain('/presets/a.toml')
  })
})

describe('the list', () => {
  it("keeps the reducer's bound, newest first, and throws nothing past it", async () => {
    await open()
    report(...Array.from({ length: 40 }, (_, i) => error('/presets/x.toml', `problem ${i}`)))

    const list = openList()
    const items = within(list).getAllByRole('listitem')
    expect(items).toHaveLength(32)
    expect(items[0].textContent).toContain('problem 39')
    expect(items[31].textContent).toContain('problem 8')
  })

  it('tells an error and a warning apart', async () => {
    await open()
    report(error('/presets/a.toml', 'a failure'), warning('/presets/b.toml', 'a complaint'))

    const list = openList()
    const items = within(list).getAllByRole('listitem')
    expect(items[0].dataset.kind).toBe('warning')
    expect(items[1].dataset.kind).toBe('error')
    expect(within(items[0]).getByText('warning')).toBeDefined()
    expect(within(items[1]).getByText('error')).toBeDefined()
  })

  it('closes again', async () => {
    await open()
    report(error('/presets/a.toml', 'one'), error('/presets/b.toml', 'two'))
    const list = openList()
    fireEvent.click(within(list).getByRole('button', { name: 'close' }))
    expect(screen.queryByRole('dialog', { name: 'problems' })).toBeNull()
  })
})
