/**
 * One palette stop on the ramp: where it sits, and what colour it is.
 *
 * A slider rather than a pointer-drag surface. It looks like a handle and it is
 * one, but the position is a `<input type="range">` underneath, which is how it
 * comes with keyboard control, a focus ring and a screen-reader name for free —
 * a `div` with `pointerdown` would have none of those and would need all three
 * written by hand and kept working.
 *
 * The same two events as a parameter: every step reports a move, and the
 * release reports a commit. Nothing is written until the release.
 */
import styles from './StopHandle.module.css'

export interface StopHandleProps {
  index: number
  at: number
  color: string
  onMove: (index: number, at: number) => void
  onCommit: (index: number, at: number) => void
  onRecolour: (index: number, color: string) => void
}

/** Two decimals is what the shipped palettes are authored at. */
const STEP = 0.01

export function StopHandle({
  index,
  at,
  color,
  onMove,
  onCommit,
  onRecolour,
}: StopHandleProps): JSX.Element {
  const commit = (): void => onCommit(index, at)
  const label = `stop ${index + 1} position`

  return (
    <div className={styles.stop}>
      <input
        className={styles.swatch}
        type="color"
        value={color}
        aria-label={`stop ${index + 1} colour`}
        // A colour input has no drag: `change` is the commit.
        onChange={(event) => onRecolour(index, event.currentTarget.value)}
      />
      <input
        className={styles.position}
        type="range"
        min={0}
        max={1}
        step={STEP}
        value={at}
        aria-label={label}
        onChange={(event) => onMove(index, event.currentTarget.valueAsNumber)}
        onPointerUp={commit}
        onKeyUp={commit}
        onBlur={commit}
      />
      <output className={styles.at} aria-label={`${label} value`}>
        {at.toFixed(2)}
      </output>
    </div>
  )
}
