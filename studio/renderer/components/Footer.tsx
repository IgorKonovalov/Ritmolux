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

/** The control cell's three parts: the address, the alarm, and the readings. */
export interface ControlReading {
  address: string
  /** The listener bound an address and is no longer reading it. */
  stopped: boolean
  title: string
}

/**
 * What the footer says about the control socket.
 *
 * `hello.control` names the address the player bound and never moves; `health`
 * carries whether the receive loop is still running, how many datagrams the
 * socket handed it and how many receives failed (ADR-0221). A player that
 * predates those readings sends a `health` line without them, so `undefined`
 * means "this player does not say" and only an explicit `false` is a listener
 * that stopped — which is the difference between a quiet player and a deaf one,
 * and the reason a click that does nothing is no longer the first sign of it.
 */
export function controlReading(
  hello: HelloEvent | undefined,
  health: HealthEvent | undefined,
): ControlReading {
  const bound = hello?.control ?? null
  const address = bound ?? 'none'
  if (health?.ctl_listening === undefined) {
    return {
      address,
      stopped: false,
      title:
        bound === null
          ? 'No control socket is bound, so this player cannot be driven'
          : 'This player does not report whether it is still listening',
    }
  }
  const counts =
    `${health.ctl_received ?? 0} datagrams received, ` +
    `${health.ctl_recv_errors ?? 0} receive errors`
  if (bound !== null && !health.ctl_listening) {
    return {
      address,
      stopped: true,
      title: `The player stopped reading its control socket (${counts}), so nothing the studio sends reaches it`,
    }
  }
  return { address, stopped: false, title: `Listening (${counts})` }
}

export function Footer({ hello, health, stats, streamLabel }: Props): JSX.Element {
  const control = controlReading(hello, health)
  return (
    <footer className={styles.footer}>
      <span className={styles.cell}>
        player <strong>{hello?.version ?? '—'}</strong>
      </span>
      <span className={styles.cell}>
        schema <code>{hello?.schema.slice(0, 8) ?? '—'}</code>
      </span>
      <span className={styles.cell} title={control.title}>
        control{' '}
        <code className={control.stopped ? styles.stopped : undefined}>{control.address}</code>
        {control.stopped && <strong className={styles.stopped}> stopped listening</strong>}
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
