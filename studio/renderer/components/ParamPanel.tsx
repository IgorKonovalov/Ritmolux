/**
 * Every parameter the active preset may bind: the ones it binds on top, the
 * rest filed by the group the engine declares for them (ADR-0256).
 *
 * **Generated, never listed.** The rows come from the schema document the
 * player printed, so a parameter added to the engine appears here with no edit
 * (ADR-0170), and so do its group and whether it is main. The rows are drawn
 * from the system's own roster followed by the engine stages, because the
 * loader accepts a stage parameter from a preset of any system — a panel
 * showing only the system's own would refuse moves the player would have taken.
 *
 * **Bound first, the rest collapsed.** What the preset binds is what its author
 * is working on, so those rows are always open. Every other row sits in one
 * `<details>` per group, in `PARAM_GROUPS` order, closed until opened, with the
 * group's `main` rows first. Which group is open is view state held by the
 * element itself and owes no setting (ADR-0240). The split is derived from the
 * bindings on every render, so a row the first release binds moves up by itself
 * once the file is re-read.
 *
 * **What the family on screen does not read is grouped, not hidden** (ADR-0194).
 * A curve preset drawing a superformula reads seven of its system's twelve
 * parameters; the other five keep their rows, bound or not, because the file
 * may bind one and a family switch makes it live again, but they sit together
 * at the end where they read as inert rather than as sliders that do nothing.
 */
import { PARAM_GROUPS, rangeFor, type ParamRoster, type ParamSpec } from '@shared/schema'
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
  const all = rosters.flatMap((roster) => roster.params)
  // One trailing group rather than one per roster: only a family-bearing
  // system's own roster ever has inert rows, and the engine stages read the
  // same whatever is drawn, so a second empty heading would say nothing.
  const inert = all.filter(isInert)
  const live = all.filter((spec) => !isInert(spec))
  const boundRows = live.filter((spec) => bound.has(spec.name))
  const groups = PARAM_GROUPS.map((group) => {
    const rows = live.filter((spec) => spec.group === group && !bound.has(spec.name))
    // Stable: declaration order within each half.
    return { group, rows: [...rows.filter((spec) => spec.main), ...rows.filter((spec) => !spec.main)] }
  }).filter(({ rows }) => rows.length > 0)

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
      {boundRows.length > 0 && (
        <section className={styles.group} data-bound="">
          <h2 className={styles.heading}>bound by this preset</h2>
          {boundRows.map(row)}
        </section>
      )}
      {groups.map(({ group, rows }) => (
        <details className={styles.fold} key={group} data-group={group}>
          <summary className={styles.summary}>
            <span className={styles.foldName}>{group}</span>
            <span className={styles.count}>{rows.length}</span>
          </summary>
          <div className={styles.rows}>{rows.map(row)}</div>
        </details>
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
