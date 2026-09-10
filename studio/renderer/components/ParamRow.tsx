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
 *
 * **What the value means decides the control.** The engine rounds a
 * `structural` parameter once before the scene sees it (ADR-0180 rule 2), so
 * offering it a continuous slider would show travel that changes nothing: three
 * quarters of a step draws the same picture as the step. Those get whole steps,
 * and the value they send is the value the engine would have rounded to — a
 * slider that reported 6.4 petals while the scene drew 6 would be the studio
 * disagreeing with the player about what it just did.
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

/**
 * `value` as the engine will receive it: rounded for a parameter whose value
 * carries an integer meaning, untouched otherwise. The same rule
 * `ParamKind::quantize` applies on the other side of the socket.
 */
function quantize(spec: ParamSpec, value: number): number {
  return spec.kind === 'structural' ? Math.round(value) : value
}

export function ParamRow({
  spec,
  binding,
  writable,
  onDrag,
  onCommit,
}: ParamRowProps): JSX.Element {
  const bound = quantize(spec, binding?.kind === 'const' ? binding.value : spec.default)
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

  const commit = (): void => {
    if (writable) onCommit(spec.name, value)
  }
  const move = (raw: number): void => {
    if (!Number.isFinite(raw)) return
    const next = quantize(spec, raw)
    setValue(next)
    onDrag(spec.name, next)
  }
  const id = `param-${spec.name}`

  // No declared bounds, so no slider: a range control needs two ends, and
  // inventing them would offer travel the engine never promised. Thirty-one
  // parameters are in this arm, the pan offsets among them.
  const unbounded = spec.range === null
  const [lo, hi] = spec.range ?? [0, 1]
  // A whole step for a structural parameter, and a two-hundredth of the range
  // for a modal one — fine enough to read as continuous, coarse enough that one
  // gesture is tens of datagrams rather than hundreds.
  const step = spec.kind === 'structural' ? 1 : (hi - lo) / STEPS

  return (
    <div className={styles.row} title={spec.doc}>
      <label className={styles.name} htmlFor={id}>
        {spec.name}
      </label>
      <input
        id={id}
        className={unbounded ? styles.field : styles.slider}
        type={unbounded ? 'number' : 'range'}
        {...(unbounded
          ? { step: spec.kind === 'structural' ? 1 : 'any' }
          : { min: lo, max: hi, step })}
        value={value}
        onChange={(event) => move(event.currentTarget.valueAsNumber)}
        // A gesture ends with a pointer release, a key release or the control
        // losing focus; each is one write and none of them fires per step.
        onPointerUp={commit}
        onKeyUp={commit}
        onBlur={commit}
      />
      {!unbounded && (
        <output className={styles.value} htmlFor={id}>
          {spec.kind === 'structural' ? value.toFixed(0) : value.toFixed(3)}
        </output>
      )}
      {binding === undefined && <span className={styles.note}>default</span>}
    </div>
  )
}
