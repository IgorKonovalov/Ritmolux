/** A refusal or a problem, said out loud rather than left as a blank preview. */
import type { ReactNode } from 'react'

import styles from './Banner.module.css'

interface Props {
  kind: 'error' | 'warning'
  title: string
  detail: string
  /**
   * What can be done about it, beside the title — the one banner that has more
   * to show than it fits puts the way in here.
   */
  action?: ReactNode
}

export function Banner({ kind, title, detail, action }: Props): JSX.Element {
  return (
    <div className={styles.banner} data-kind={kind} role="alert">
      <div className={styles.head}>
        <strong className={styles.title}>{title}</strong>
        {action}
      </div>
      <span className={styles.detail}>{detail}</span>
    </div>
  )
}
