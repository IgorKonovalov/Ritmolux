/**
 * The prompt timeline the Render view builds and `sd_filter.py --timeline`
 * reads: `[{at_bar, prompt}]`, ascending and unique (ADR-0236).
 *
 * The validator refuses what the sidecar's `parse_timeline` and
 * `check_timeline_fits` refuse, so a timeline the view accepts is one the
 * sidecar starts on rather than one it rejects an hour into a render. The edit
 * helpers keep the invariant by construction: each returns a new list that is
 * still ascending and unique.
 */
import type { BarGrid } from './render'

export interface TimelineEntry {
  /** 1-based bar the prompt is fully reached on. */
  at_bar: number
  prompt: string
}

/**
 * Why `timeline` would be refused, or `undefined` when it would not.
 *
 * `bars` is the grid's bar count; an entry at or past `bars + 1` is past the
 * track. Each refusal names the entry the way the sidecar does, by its 1-based
 * place in the list.
 */
export function timelineProblem(timeline: TimelineEntry[], bars?: number): string | undefined {
  if (timeline.length === 0) return 'the timeline needs at least one prompt'
  for (const [index, entry] of timeline.entries()) {
    const where = `prompt ${index + 1}`
    if (!Number.isFinite(entry.at_bar)) return `${where}: its bar is not a number`
    if (entry.at_bar < 1) return `${where}: bars count from 1, and it is at bar ${entry.at_bar}`
    if (entry.prompt.trim() === '') return `${where} at bar ${entry.at_bar} is empty`
    const previous = timeline[index - 1]
    if (previous !== undefined && entry.at_bar <= previous.at_bar) {
      return `${where}: bar ${entry.at_bar} is not after the previous prompt's bar ${previous.at_bar}`
    }
    if (bars !== undefined && entry.at_bar >= bars + 1) {
      return `${where}: bar ${entry.at_bar} is past the track, whose last bar is bar ${bars}`
    }
  }
  return undefined
}

/** The bar whose start is nearest `frame`, 1-based. */
export function nearestBar(grid: BarGrid, frame: number): number {
  let best = 0
  for (let index = 1; index < grid.bar_starts.length; index += 1) {
    if (Math.abs(grid.bar_starts[index] - frame) < Math.abs(grid.bar_starts[best] - frame)) {
      best = index
    }
  }
  return best + 1
}

function sorted(timeline: TimelineEntry[]): TimelineEntry[] {
  return [...timeline].sort((a, b) => a.at_bar - b.at_bar)
}

/** Add an empty prompt at `bar`, unless one is there already. */
export function addEntry(timeline: TimelineEntry[], bar: number): TimelineEntry[] {
  if (timeline.some((entry) => entry.at_bar === bar)) return timeline
  return sorted([...timeline, { at_bar: bar, prompt: '' }])
}

/** Move the prompt at `from` to `to`, unless `to` already holds one. */
export function moveEntry(timeline: TimelineEntry[], from: number, to: number): TimelineEntry[] {
  if (from === to || timeline.some((entry) => entry.at_bar === to)) return timeline
  return sorted(timeline.map((entry) => (entry.at_bar === from ? { ...entry, at_bar: to } : entry)))
}

export function setPrompt(timeline: TimelineEntry[], bar: number, prompt: string): TimelineEntry[] {
  return timeline.map((entry) => (entry.at_bar === bar ? { ...entry, prompt } : entry))
}

export function removeEntry(timeline: TimelineEntry[], bar: number): TimelineEntry[] {
  return timeline.filter((entry) => entry.at_bar !== bar)
}
