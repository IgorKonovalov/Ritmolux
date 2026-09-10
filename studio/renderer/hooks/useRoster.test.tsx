/**
 * @vitest-environment jsdom
 *
 * A click asks, and the player answers (Plan 0159 Phase 8).
 *
 * `ctl/preset` dissolves, so for the length of a crossfade the player still
 * reports the outgoing preset. The highlight has to follow what the player says
 * is on screen rather than what was clicked, or the list would claim a switch
 * that a name outside the roster never caused.
 */
import { act, renderHook } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { useRoster } from './useRoster'

const NAMES = ['Aurora', 'Clifford', 'Ember']

describe('selecting from the library', () => {
  it('sends the name and marks it pending until the player confirms', () => {
    const send = vi.fn()
    const { result, rerender } = renderHook(
      ({ active }: { active: string }) => useRoster(NAMES, active, send),
      { initialProps: { active: 'Aurora' } },
    )

    act(() => result.current.select('Ember'))
    expect(send).toHaveBeenCalledWith('Ember')
    // Still Aurora on screen: the crossfade has not finished.
    expect(result.current.active).toBe('Aurora')
    expect(result.current.pending).toBe('Ember')

    rerender({ active: 'Ember' })
    expect(result.current.pending).toBeUndefined()
    expect(result.current.active).toBe('Ember')
  })

  it('clears the wait even when the player reports the preset it was leaving', () => {
    // An unknown name changes nothing at the player, so the confirmation the
    // click waits for never names it. A pending mark that waited for it would
    // stay lit for the rest of the session.
    const send = vi.fn()
    const { result, rerender } = renderHook(
      ({ active }: { active: string }) => useRoster(NAMES, active, send),
      { initialProps: { active: 'Aurora' } },
    )
    act(() => result.current.select('Nothing'))
    expect(result.current.pending).toBe('Nothing')
    rerender({ active: 'Clifford' })
    expect(result.current.pending).toBeUndefined()
  })

  it('sends nothing when the entry clicked is the one already on screen', () => {
    const send = vi.fn()
    const { result } = renderHook(() => useRoster(NAMES, 'Aurora', send))
    act(() => result.current.select('Aurora'))
    expect(send).not.toHaveBeenCalled()
    expect(result.current.pending).toBeUndefined()
  })

  it('carries the roster through in the player own order', () => {
    const { result } = renderHook(() => useRoster(NAMES, 'Aurora', vi.fn()))
    expect(result.current.names).toEqual(NAMES)
  })
})
