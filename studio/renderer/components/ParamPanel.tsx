/**
 * Every parameter the active preset may bind, grouped by the roster that
 * declares it.
 *
 * **Generated, never listed.** The rows come from the schema document the
 * player printed, so a parameter added to the engine appears here with no edit
 * (ADR-0170). The groups are the system's own roster followed by the engine
 * stages, because the loader accepts a stage parameter from a preset of any
 * system — a panel showing only the system's own would refuse moves the player
 * would have taken.
 *
 * **What the family on screen does not read is grouped, not hidden** (ADR-0194).
 * A curve preset drawing a superformula reads seven of its system's twelve
 * parameters; the other five keep their rows, because the file may bind one and
 * a family switch makes it live again, but they sit together at the end where
 * they read as inert rather than as sliders that do nothing.
 */
import { rangeFor, type ParamRoster, type ParamSpec } from '@shared/schema'
import type { Binding } from '@shared/toml'

import { ParamRow } from './ParamRow'

import styles from './ParamPanel.module.css'

export interface ParamPanelProps {
  rosters: ParamRoster[]
  /** The family the preset on screen draws, or `null`/`undefined` for none. */
  family: string | null | undefined
  bindings: Binding[]
  writable: boolean
  /** False while the preset document is unknown — see `ParamRow`. */
  hasDocument: boolean
  onDrag: (name: string, value: number) => void
  onCommit: (name: string, value: number) => void
}

export function ParamPanel({
  rosters,
  family,
  bindings,
  writable,
  hasDocument,
  onDrag,
  onCommit,
}: ParamPanelProps): JSX.Element {
  const bound = new Map(bindings.map((binding) => [binding.name, binding]))
  const isInert = (spec: ParamSpec): boolean => rangeFor(spec, family).inert
  // One trailing group rather than one per roster: only a family-bearing
  // system's own roster ever has inert rows, and the engine stages read the
  // same whatever is drawn, so a second empty heading would say nothing.
  const inert = rosters.flatMap((roster) => roster.params.filter(isInert))

  const row = (spec: ParamSpec): JSX.Element => (
    <ParamRow
      key={spec.name}
      spec={spec}
      family={family}
      binding={bound.get(spec.name)}
      writable={writable}
      hasDocument={hasDocument}
      onDrag={onDrag}
      onCommit={onCommit}
    />
  )

  return (
    <div className={styles.panel}>
      {rosters.map((roster) => (
        <section className={styles.group} key={roster.name}>
          <h2 className={styles.heading}>{roster.name}</h2>
          {roster.params.filter((spec) => !isInert(spec)).map(row)}
        </section>
      ))}
      {inert.length > 0 && (
        <section className={styles.group}>
          <h2 className={styles.heading}>not read on {family}</h2>
          {inert.map(row)}
        </section>
      )}
    </div>
  )
}
