/**
 * One parameter: what the engine declares it is, and what this preset binds it
 * to.
 *
 * Three shapes, decided by the binding rather than by the parameter:
 * a **constant** is a slider, a bound **expression** is read-only text because
 * the file is what drives it, and an **unbound** parameter is a slider sitting
 * at the engine's default — dragging it writes the binding the preset did not
 * have.
 *
 * The drag and the release are different events on purpose. Every step sends
 * `ctl/param`, which the player applies on its next frame and holds; only the
 * release writes the file. So the picture follows the finger and the disk sees
 * one write per gesture.
 */
import { useEffect, useState } from 'react'

import type { ParamSpec } from '@shared/schema'
import type { Binding } from '@shared/toml'

import styles from './ParamRow.module.css'

export interface ParamRowProps {
  spec: ParamSpec
  /** The preset's own binding, or `undefined` when it does not bind this. */
  binding: Binding | undefined
  /**
   * False when there is no file behind the preset. The slider still drags —
   * the override moves the picture either way — and the release writes
   * nothing, which is what an embedded preset can honestly offer.
   */
  writable: boolean
  onDrag: (name: string, value: number) => void
  onCommit: (name: string, value: number) => void
}

/**
 * A slider step fine enough that a drag reads as continuous across any range
 * the engine declares, and coarse enough that one gesture is tens of datagrams
 * rather than hundreds. The written value is rounded to four decimals, so a
 * step below that would write nothing new.
 */
const STEPS = 200

export function ParamRow({
  spec,
  binding,
  writable,
  onDrag,
  onCommit,
}: ParamRowProps): JSX.Element {
  const bound = binding?.kind === 'const' ? binding.value : spec.default
  const [value, setValue] = useState(bound)

  // The file is the durable channel: when a reload brings a different value in,
  // the slider follows it rather than holding what this window last dragged.
  useEffect(() => setValue(bound), [bound])

  if (binding?.kind === 'expr') {
    return (
      <div className={styles.row} title={spec.doc}>
        <span className={styles.name}>{spec.name}</span>
        <code className={styles.expr}>{binding.text}</code>
        <span className={styles.note}>expression</span>
      </div>
    )
  }

  if (binding?.kind === 'opaque') {
    return (
      <div className={styles.row} title={spec.doc}>
        <span className={styles.name}>{spec.name}</span>
        <code className={styles.expr}>{binding.text}</code>
        <span className={styles.note}>not editable here</span>
      </div>
    )
  }

  const [lo, hi] = spec.range
  const step = (hi - lo) / STEPS
  const commit = (): void => {
    if (writable) onCommit(spec.name, value)
  }

  return (
    <div className={styles.row} title={spec.doc}>
      <label className={styles.name} htmlFor={`param-${spec.name}`}>
        {spec.name}
      </label>
      <input
        id={`param-${spec.name}`}
        className={styles.slider}
        type="range"
        min={lo}
        max={hi}
        step={step}
        value={value}
        onChange={(event) => {
          const next = event.currentTarget.valueAsNumber
          setValue(next)
          onDrag(spec.name, next)
        }}
        // A gesture ends with a pointer release, a key release or the control
        // losing focus; each is one write and none of them fires per step.
        onPointerUp={commit}
        onKeyUp={commit}
        onBlur={commit}
      />
      <output className={styles.value} htmlFor={`param-${spec.name}`}>
        {value.toFixed(3)}
      </output>
      {binding === undefined && <span className={styles.note}>default</span>}
    </div>
  )
}
