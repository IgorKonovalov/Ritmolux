/**
 * The reading strip: what is driving the picture, and what it cost.
 *
 * The dropped count is shown because a dropped preview frame is a **normal
 * reading**, not a defect (ADR-0178) — a number with no home on screen is a
 * number a user chases.
 */
import type { HealthEvent, HelloEvent } from '@shared/protocol'

import type { PreviewStats } from './Preview'

import styles from './Footer.module.css'

interface Props {
  hello: HelloEvent | undefined
  health: HealthEvent | undefined
  stats: PreviewStats
  streamLabel: string | undefined
}

export function Footer({ hello, health, stats, streamLabel }: Props): JSX.Element {
  return (
    <footer className={styles.footer}>
      <span className={styles.cell}>
        player <strong>{hello?.version ?? '—'}</strong>
      </span>
      <span className={styles.cell}>
        schema <code>{hello?.schema.slice(0, 8) ?? '—'}</code>
      </span>
      <span className={styles.cell}>
        control <code>{hello?.control ?? 'none'}</code>
      </span>
      <span className={styles.cell}>stream {streamLabel ?? '—'}</span>
      <span className={styles.spacer} />
      {health !== undefined && (
        <span className={styles.cell}>
          {health.fps.toFixed(1)} fps · p99 {health.frame_ms_p99.toFixed(1)} ms
        </span>
      )}
      <span className={styles.cell} title="Frames the preview did not keep up with">
        {stats.delivered} painted · {stats.dropped} dropped
      </span>
    </footer>
  )
}
