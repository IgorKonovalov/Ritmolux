/** A refusal or a problem, said out loud rather than left as a blank preview. */
import styles from './Banner.module.css'

interface Props {
  kind: 'error' | 'warning'
  title: string
  detail: string
}

export function Banner({ kind, title, detail }: Props): JSX.Element {
  return (
    <div className={styles.banner} data-kind={kind} role="alert">
      <strong className={styles.title}>{title}</strong>
      <span className={styles.detail}>{detail}</span>
    </div>
  )
}
