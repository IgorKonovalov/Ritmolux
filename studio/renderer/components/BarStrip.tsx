/**
 * The track's waveform with the player's bars over it, and the prompts placed
 * on those bars (ADR-0262, ADR-0236).
 *
 * The bars are `--bars`' own grid, the one the render and the sidecar read, so
 * a prompt placed here lands on a bar the run has. A bar the downbeat estimator
 * placed is drawn apart from one the fallback counter placed: most of a grid is
 * fallback, and the strip says so rather than implying a phrase boundary.
 *
 * Between two prompts the strip shades a blend, because the sidecar
 * interpolates the conditioning across the whole span rather than switching at
 * the marker; before the first prompt the first holds, after the last the last.
 *
 * Positions are fractions of the track, so the strip scales with its width and
 * no geometry is measured except while a marker is dragged.
 */
import { useEffect, useRef, useState } from 'react'

import type { BarGrid, Peaks } from '@shared/render'
import {
  addEntry,
  moveEntry,
  nearestBar,
  removeEntry,
  setPrompt,
  type TimelineEntry,
} from '@shared/timeline'

import styles from './BarStrip.module.css'

interface Props {
  grid: BarGrid
  peaks: Peaks
  /** Ascending and unique; every edit hands back a list that still is. */
  timeline: TimelineEntry[]
  onChange: (timeline: TimelineEntry[]) => void
  disabled?: boolean
}

/** The hues prompts cycle through; a blend runs from one to the next. */
const HUES = [196, 32, 286, 128]

function tint(index: number, alpha: number): string {
  return `hsl(${HUES[index % HUES.length]} 80% 60% / ${alpha})`
}

function percent(frame: number, frames: number): string {
  return `${(Math.min(Math.max(frame, 0), frames) / frames) * 100}%`
}

/** One closed outline: the highs left to right, the lows back. */
function outline(peaks: Peaks): string {
  const top = peaks.max.map((value, column) => `${column},${1 - value}`)
  const bottom = peaks.min.map((value, column) => `${column + 1},${1 - value}`).reverse()
  return [...top, ...bottom].join(' ')
}

export function BarStrip({ grid, peaks, timeline, onChange, disabled = false }: Props): JSX.Element {
  const lane = useRef<HTMLDivElement>(null)
  /** The marker being dragged, and the bar it would land on now. */
  const [drag, setDrag] = useState<{ from: number; over: number }>()

  const bars = grid.bar_starts.length
  const downbeats = grid.bar_locked.filter(Boolean).length
  const startOf = (bar: number): number => grid.bar_starts[bar - 1] ?? grid.frames

  useEffect(() => {
    if (drag === undefined) return
    const barAt = (clientX: number): number => {
      const box = lane.current?.getBoundingClientRect()
      if (box === undefined || box.width <= 0) return drag.from
      const frame = ((clientX - box.left) / box.width) * grid.frames
      return nearestBar(grid, frame)
    }
    const move = (event: MouseEvent): void => {
      const over = barAt(event.clientX)
      setDrag((current) => (current === undefined ? current : { ...current, over }))
    }
    const up = (event: MouseEvent): void => {
      onChange(moveEntry(timeline, drag.from, barAt(event.clientX)))
      setDrag(undefined)
    }
    window.addEventListener('mousemove', move)
    window.addEventListener('mouseup', up)
    return () => {
      window.removeEventListener('mousemove', move)
      window.removeEventListener('mouseup', up)
    }
  }, [drag, grid, timeline, onChange])

  /** Where each prompt is drawn: its own bar, or the one it is being dragged to. */
  const shown = timeline.map((entry) =>
    drag !== undefined && entry.at_bar === drag.from ? { ...entry, at_bar: drag.over } : entry,
  )
  const spans = shown.map((entry, index) => {
    const next = shown[index + 1]
    const from = index === 0 ? 0 : startOf(entry.at_bar)
    const to = next === undefined ? grid.frames : startOf(next.at_bar)
    const background =
      next === undefined
        ? tint(index, 0.18)
        : `linear-gradient(to right, ${tint(index, 0.22)}, ${tint(index + 1, 0.22)})`
    return { key: entry.at_bar, left: percent(from, grid.frames), width: percent(to - from, grid.frames), background }
  })

  return (
    <div className={styles.strip}>
      <div className={styles.lane} ref={lane} data-testid="bar-lane">
        {spans.map((span) => (
          <div
            key={span.key}
            className={styles.blend}
            style={{ left: span.left, width: span.width, background: span.background }}
            aria-hidden="true"
          />
        ))}
        <svg
          className={styles.wave}
          viewBox={`0 0 ${Math.max(peaks.max.length, 1)} 2`}
          preserveAspectRatio="none"
          aria-hidden="true"
        >
          <polygon points={outline(peaks)} />
        </svg>
        {grid.bar_starts.map((start, index) => (
          <button
            key={start}
            type="button"
            className={`${styles.bar} ${grid.bar_locked[index] ? styles.locked : styles.fallback}`}
            data-locked={grid.bar_locked[index]}
            style={{ left: percent(start, grid.frames) }}
            aria-label={`bar ${index + 1}${grid.bar_locked[index] ? ', on a downbeat' : ''}`}
            title={`bar ${index + 1}`}
            disabled={disabled}
            onClick={() => onChange(addEntry(timeline, index + 1))}
          />
        ))}
        {shown.map((entry, index) => (
          <div
            key={timeline[index].at_bar}
            className={`${styles.marker} ${entry.at_bar > bars ? styles.past : ''}`}
            style={{ left: percent(startOf(entry.at_bar), grid.frames), borderColor: tint(index, 1) }}
            role="button"
            tabIndex={-1}
            aria-label={`prompt at bar ${entry.at_bar}`}
            onMouseDown={(event) => {
              if (disabled || event.button !== 0) return
              event.preventDefault()
              setDrag({ from: timeline[index].at_bar, over: timeline[index].at_bar })
            }}
          >
            <span className={styles.flag} style={{ background: tint(index, 1) }}>
              {entry.at_bar}
            </span>
          </div>
        ))}
      </div>
      <p className={styles.summary}>
        {bars} bars, {downbeats} on a downbeat
      </p>
      <ol className={styles.prompts}>
        {timeline.map((entry, index) => (
          <li key={entry.at_bar} className={styles.prompt}>
            <label className={styles.promptLabel}>
              <span className={styles.swatch} style={{ background: tint(index, 1) }} />
              <span className={styles.promptBar}>bar {entry.at_bar}</span>
              <input
                className={styles.promptText}
                value={entry.prompt}
                placeholder="what to draw from this bar on"
                disabled={disabled}
                onChange={(event) => onChange(setPrompt(timeline, entry.at_bar, event.target.value))}
              />
            </label>
            <button
              type="button"
              className={styles.remove}
              disabled={disabled}
              onClick={() => onChange(removeEntry(timeline, entry.at_bar))}
            >
              remove
            </button>
          </li>
        ))}
      </ol>
    </div>
  )
}
