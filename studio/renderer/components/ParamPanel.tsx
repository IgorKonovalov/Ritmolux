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
 */
import type { ParamRoster } from '@shared/schema'
import type { Binding } from '@shared/toml'

import { ParamRow } from './ParamRow'

import styles from './ParamPanel.module.css'

export interface ParamPanelProps {
  rosters: ParamRoster[]
  bindings: Binding[]
  writable: boolean
  onDrag: (name: string, value: number) => void
  onCommit: (name: string, value: number) => void
}

export function ParamPanel({
  rosters,
  bindings,
  writable,
  onDrag,
  onCommit,
}: ParamPanelProps): JSX.Element {
  const bound = new Map(bindings.map((binding) => [binding.name, binding]))

  return (
    <div className={styles.panel}>
      {rosters.map((roster) => (
        <section className={styles.group} key={roster.name}>
          <h2 className={styles.heading}>{roster.name}</h2>
          {roster.params.map((spec) => (
            <ParamRow
              key={spec.name}
              spec={spec}
              binding={bound.get(spec.name)}
              writable={writable}
              onDrag={onDrag}
              onCommit={onCommit}
            />
          ))}
        </section>
      ))}
    </div>
  )
}
