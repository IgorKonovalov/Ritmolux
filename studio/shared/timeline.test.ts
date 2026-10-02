/**
 * The timeline validator refuses what `sd_filter.py`'s `parse_timeline`
 * refuses (Plan 0247 Phase 3), and the edits keep a timeline ascending and
 * unique.
 */
import { describe, expect, it } from 'vitest'

import type { BarGrid } from './render'
import {
  addEntry,
  moveEntry,
  nearestBar,
  removeEntry,
  setPrompt,
  timelineProblem,
  type TimelineEntry,
} from './timeline'

const at = (at_bar: number, prompt = 'a canyon of luminous rock'): TimelineEntry => ({ at_bar, prompt })

describe('the timeline validator', () => {
  it.each<[string, TimelineEntry[], number | undefined, RegExp]>([
    ['an empty list', [], 16, /at least one prompt/],
    ['a bar below 1', [at(0)], 16, /bars count from 1/],
    ['a negative bar', [at(-3)], 16, /bars count from 1/],
    ['a bar that is not a number', [at(Number.NaN)], 16, /not a number/],
    ['a non-ascending bar', [at(5), at(3)], 16, /prompt 2: bar 3 is not after .* bar 5/],
    ['a duplicate bar', [at(1), at(9), at(9)], 16, /prompt 3: bar 9 is not after/],
    ['an empty prompt', [at(1), at(4, '')], 16, /prompt 2 at bar 4 is empty/],
    ['a blank prompt', [at(1, '   ')], 16, /prompt 1 at bar 1 is empty/],
    ['a bar past the grid', [at(1), at(17)], 16, /bar 17 is past the track, whose last bar is bar 16/],
  ])('refuses %s', (_case, timeline, bars, message) => {
    expect(timelineProblem(timeline, bars)).toMatch(message)
  })

  it.each<[string, TimelineEntry[], number | undefined]>([
    ['one prompt on bar 1', [at(1)], 16],
    ['prompts up to the last bar', [at(1), at(8), at(16)], 16],
    ['any bar when the grid is not known', [at(1), at(400)], undefined],
  ])('accepts %s', (_case, timeline, bars) => {
    expect(timelineProblem(timeline, bars)).toBeUndefined()
  })
})

describe('the edits', () => {
  const GRID: BarGrid = { fps: '30', frames: 400, bar_starts: [0, 96, 192, 288], bar_locked: [false, false, true, false] }

  it('snaps a frame to the bar whose start is nearest', () => {
    expect(nearestBar(GRID, 0)).toBe(1)
    expect(nearestBar(GRID, 47)).toBe(1)
    expect(nearestBar(GRID, 50)).toBe(2)
    expect(nearestBar(GRID, 399)).toBe(4)
  })

  it('keeps the list ascending and unique', () => {
    let timeline = [at(1, 'first')]
    timeline = addEntry(timeline, 3)
    timeline = addEntry(timeline, 2)
    timeline = addEntry(timeline, 2)
    expect(timeline.map((entry) => entry.at_bar)).toEqual([1, 2, 3])

    timeline = moveEntry(timeline, 1, 4)
    expect(timeline).toEqual([at(2, ''), at(3, ''), at(4, 'first')])
    // A bar that already holds a prompt is refused rather than merged.
    expect(moveEntry(timeline, 2, 3)).toBe(timeline)

    timeline = setPrompt(timeline, 3, 'a rose window')
    timeline = removeEntry(timeline, 2)
    expect(timeline).toEqual([at(3, 'a rose window'), at(4, 'first')])
  })
})
