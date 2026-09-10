/**
 * The studio's own settings — the choices that belong to this machine rather
 * than to the preset on screen.
 *
 * Which is why it is not a sixth editor tab: the player mode outlives every
 * preset the window opens, and putting it beside `parameters` would say it was
 * a property of the look.
 *
 * The mode is read when the player is spawned (ADR-0186), so a change here is
 * written to the settings file and applied by the next launch. That is stated
 * on the surface rather than implied: a control that appears to do nothing is
 * worse than one that says when it will.
 */
import { useState } from 'react'

import { PLAYER_MODES, type PlayerMode } from '@shared/player-mode'

import styles from './Settings.module.css'

export interface SettingsProps {
  /** The mode the running player was actually spawned in. */
  running: PlayerMode
  playerPath: string | undefined
  playerSource: string | undefined
  studioVersion: string
  onClose: () => void
}

const WHAT_IT_DOES: Record<PlayerMode, string> = {
  windowed:
    'The player opens its own show window and this preview is a copy of what that window draws.',
  windowless:
    'The player opens no window. This preview is the only picture there is — it is not a feed of what an audience can see.',
}

export function Settings({
  running,
  playerPath,
  playerSource,
  studioVersion,
  onClose,
}: SettingsProps): JSX.Element {
  /**
   * The mode the user picked in this window, or `undefined` while they have
   * picked none.
   *
   * Not seeded from `running`: `running` arrives with the app info, one IPC
   * round trip after the first render, and a `useState(running)` would keep
   * whatever the default was at mount and then report a choice nobody made.
   */
  const [picked, setPicked] = useState<PlayerMode>()
  const [problem, setProblem] = useState<string>()
  const chosen = picked ?? running

  const choose = (mode: PlayerMode): void => {
    setPicked(mode)
    void window.api.app.setPlayerMode(mode).then((result) => {
      // The choice is kept on screen either way; what changes is whether the
      // file took it, which is the only thing the next launch reads.
      setProblem(result.ok ? undefined : result.reason)
    })
  }

  return (
    <section className={styles.panel} aria-label="Studio settings">
      <div className={styles.bar}>
        <h2 className={styles.heading}>Settings</h2>
        <button type="button" className={styles.close} onClick={onClose}>
          close
        </button>
      </div>

      <fieldset className={styles.group}>
        <legend className={styles.legend}>Player mode</legend>
        {PLAYER_MODES.map((mode) => (
          <label key={mode} className={styles.choice}>
            <input
              type="radio"
              name="player-mode"
              value={mode}
              checked={chosen === mode}
              onChange={() => choose(mode)}
            />
            <span className={styles.choiceName}>{mode}</span>
            <span className={styles.choiceDoc}>{WHAT_IT_DOES[mode]}</span>
          </label>
        ))}
        <p className={styles.note}>
          {picked === undefined || picked === running
            ? `The player is running in ${running}.`
            : `Saved. The player is still running in ${running} — reopen the studio to start it in ${picked}.`}
        </p>
        {problem !== undefined && (
          <p className={styles.problem} role="alert">
            The setting was not written: {problem}
          </p>
        )}
      </fieldset>

      <dl className={styles.facts}>
        <dt>player</dt>
        <dd>
          <code>{playerPath ?? 'none found'}</code>
          {playerSource !== undefined && <span className={styles.source}> ({playerSource})</span>}
        </dd>
        <dt>studio</dt>
        <dd>
          <code>{studioVersion}</code>
        </dd>
      </dl>
    </section>
  )
}
