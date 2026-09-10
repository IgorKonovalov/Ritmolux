/**
 * @vitest-environment jsdom
 *
 * Rotation is held while the studio is attached, and given back on request
 * (Plan 0168 Phase 1).
 *
 * The second assertion is the interesting one: a re-attach sends `hold` again
 * rather than the opposite verb. `auto` and `hold` are **positions, not
 * presses** (spec 0003), and this is the first consumer to depend on that for
 * correctness — a surface that toggled would hand rotation back to a show
 * whose preset is under an open editor.
 */
import { act, cleanup, fireEvent, render, renderHook, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { HelloEvent } from '@shared/protocol'

import { Rotation } from '../components/Rotation'
import { useHeldRotation } from './useHeldRotation'

afterEach(cleanup)

function hello(version = '0.115.0'): HelloEvent {
  return { v: 1, ev: 'hello', version, schema: 'abc', control: '127.0.0.1:9500' }
}

describe('attaching', () => {
  it('sends nothing until the player greets', () => {
    const transport = vi.fn()
    const { result } = renderHook(
      ({ event }: { event: HelloEvent | undefined }) => useHeldRotation(event, transport),
      { initialProps: { event: undefined } },
    )
    expect(transport).not.toHaveBeenCalled()
    expect(result.current.held).toBe(false)
  })

  it('holds rotation once, and repeats the position rather than toggling it', () => {
    const transport = vi.fn()
    const { result, rerender } = renderHook(
      ({ event }: { event: HelloEvent | undefined }) => useHeldRotation(event, transport),
      { initialProps: { event: undefined as HelloEvent | undefined } },
    )

    rerender({ event: hello() })
    expect(transport.mock.calls).toEqual([['hold']])
    expect(result.current.held).toBe(true)

    // A re-render with the same event is not a second attach.
    rerender({ event: hello() })
    expect(transport.mock.calls).toEqual([['hold'], ['hold']])
    expect(transport).not.toHaveBeenCalledWith('auto')
    expect(result.current.held).toBe(true)
  })

  it('gives rotation back with the opposite position', () => {
    const transport = vi.fn()
    // One object for the run, as the event reducer keeps it: the attach is the
    // event's identity, and a fresh one per render would be a fresh attach.
    const attached = hello()
    const { result } = renderHook(() => useHeldRotation(attached, transport))
    expect(result.current.held).toBe(true)

    act(() => result.current.resume())
    expect(transport).toHaveBeenLastCalledWith('auto')
    expect(result.current.held).toBe(false)
  })
})

describe('what the surface says', () => {
  it('says rotation is held, and offers it back', () => {
    const onResume = vi.fn()
    render(<Rotation held onResume={onResume} />)
    expect(screen.getByText('rotation held')).toBeDefined()
    fireEvent.click(screen.getByRole('button', { name: 'resume' }))
    expect(onResume).toHaveBeenCalledTimes(1)
  })

  it('says rotation is running, with nothing left to resume', () => {
    render(<Rotation held={false} onResume={vi.fn()} />)
    expect(screen.getByText('rotation running')).toBeDefined()
    expect(screen.queryByRole('button', { name: 'resume' })).toBeNull()
  })
})
