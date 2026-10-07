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
import { useEffect, useState } from 'react'

import { PLAYER_MODES, type PlayerMode } from '@shared/player-mode'
import type { RenderSettingKey, RenderSettings } from '@shared/render'

import styles from './Settings.module.css'

export interface SettingsProps {
  /** The mode the running player was actually spawned in. */
  running: PlayerMode
  playerPath: string | undefined
  playerSource: string | undefined
  studioVersion: string
  /** `ui.reducedMotion` as it applies now. */
  reducedMotion: boolean
  /** Apply a new `ui.reducedMotion` to the window; called once the file took it. */
  onReducedMotion: (on: boolean) => void
  /** The `render` key as the settings file held it at launch. */
  render?: RenderSettings
  onClose: () => void
}

/** The `render` keys the panel edits, each a path, and what absent means. */
const RENDER_FIELDS: { key: RenderSettingKey; label: string; absent: string }[] = [
  { key: 'ffmpegPath', label: 'ffmpeg', absent: 'ffmpeg on PATH' },
  { key: 'outputDir', label: 'output folder', absent: 'your Videos folder' },
  { key: 'diffusion.python', label: 'diffusion python', absent: 'not set: no neural renders' },
  { key: 'diffusion.script', label: 'diffusion script', absent: 'not set: no neural renders' },
]

/** One key's value as the settings file holds it, `diffusion.*` read nested. */
function fileValue(render: RenderSettings, key: RenderSettingKey): string | undefined {
  switch (key) {
    case 'ffmpegPath':
      return render.ffmpegPath
    case 'outputDir':
      return render.outputDir
    case 'diffusion.python':
      return render.diffusion?.python
    case 'diffusion.script':
      return render.diffusion?.script
  }
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
  reducedMotion,
  onReducedMotion,
  render,
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
  const [motionProblem, setMotionProblem] = useState<string>()
  const chosen = picked ?? running

  const setMotion = (on: boolean): void => {
    void window.api.app.setReducedMotion(on).then((result) => {
      // Applied only once the file took it, so the window never shows a choice
      // the next launch will not read.
      if (result.ok) onReducedMotion(on)
      setMotionProblem(result.ok ? undefined : result.reason)
    })
  }

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

      <fieldset className={styles.group}>
        <legend className={styles.legend}>Motion</legend>
        <label className={styles.choice}>
          <input
            type="checkbox"
            name="reduced-motion"
            checked={reducedMotion}
            onChange={(event) => setMotion(event.target.checked)}
          />
          <span className={styles.choiceName}>reduce motion</span>
          <span className={styles.choiceDoc}>
            Panels, tabs and dialogs appear at once instead of fading in. The system&apos;s own
            reduced-motion preference does the same whatever this says. Saved as{' '}
            <code>ui.reducedMotion</code> and applied now.
          </span>
        </label>
        {motionProblem !== undefined && (
          <p className={styles.problem} role="alert">
            The setting was not written: {motionProblem}
          </p>
        )}
      </fieldset>

      <RenderGroup render={render ?? {}} />

      <JudgingGroup />

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

/**
 * `judging.sourceDir`, the directory a judging session draws its sets from
 * (ADR-0267): a text field saved on its button, or a directory dialog that
 * writes what it picked. Read from main on mount, because the app info the
 * other groups start from does not carry it.
 */
function JudgingGroup(): JSX.Element {
  /** The file's value; `undefined` until main has answered. */
  const [held, setHeld] = useState<string | null>()
  const [draft, setDraft] = useState<string>()
  const [problem, setProblem] = useState<string>()

  useEffect(() => {
    let live = true
    void window.api.judging.getState().then((state) => {
      if (live) setHeld(state.sourceDir)
    })
    return () => {
      live = false
    }
  }, [])

  const took = (result: { ok: true; value: string | null } | { ok: false; reason: string }): void => {
    setProblem(result.ok ? undefined : result.reason)
    if (!result.ok) return
    setHeld(result.value)
    setDraft(undefined)
  }

  const save = (): void => {
    const next = (draft ?? held ?? '').trim()
    void window.api.judging.setSourceDir(next === '' ? null : next).then(took)
  }

  const choose = (): void => {
    void window.api.judging.pickSourceDir().then((result) => {
      if (result !== null) took(result)
    })
  }

  return (
    <fieldset className={styles.group}>
      <legend className={styles.legend}>Judging</legend>
      <div className={styles.path}>
        <label className={styles.pathLabel}>
          <span className={styles.choiceName}>source directory</span>
          <input
            className={styles.pathInput}
            value={draft ?? held ?? ''}
            placeholder="not set: no sets to judge"
            onChange={(event) => setDraft(event.target.value)}
          />
        </label>
        <button type="button" className={styles.close} onClick={choose}>
          choose…
        </button>
        <button type="button" className={styles.close} onClick={save}>
          save
        </button>
      </div>
      <p className={styles.note}>
        Saved as <code>judging.sourceDir</code>. The Judge view draws its sets from it: a family,
        its <code>proposed/</code> directory, or a list of its files.
      </p>
      {problem !== undefined && (
        <p className={styles.problem} role="alert">
          The setting was not written: {problem}
        </p>
      )}
    </fieldset>
  )
}

/**
 * The `render` keys, each a text field saved on its own button. An empty field
 * removes the key, which is how a path is given back to its default.
 */
function RenderGroup({ render }: { render: RenderSettings }): JSX.Element {
  /** What each field holds now, starting from the file; `undefined` while untouched. */
  const [drafts, setDrafts] = useState<Partial<Record<RenderSettingKey, string>>>({})
  /** What the file took this session; an `undefined` value is a cleared key. */
  const [saved, setSaved] = useState<Partial<Record<RenderSettingKey, string | undefined>>>({})
  const [problem, setProblem] = useState<string>()

  const value = (key: RenderSettingKey): string =>
    drafts[key] ?? (key in saved ? (saved[key] ?? '') : (fileValue(render, key) ?? ''))

  const save = (key: RenderSettingKey): void => {
    const next = value(key).trim()
    void window.api.render.setSettings({ [key]: next === '' ? null : next }).then((result) => {
      setProblem(result.ok ? undefined : result.reason)
      if (!result.ok) return
      setSaved((previous) => ({ ...previous, [key]: next === '' ? undefined : next }))
      setDrafts((previous) => ({ ...previous, [key]: undefined }))
    })
  }

  return (
    <fieldset className={styles.group}>
      <legend className={styles.legend}>Rendering</legend>
      {RENDER_FIELDS.map(({ key, label, absent }) => (
        <div key={key} className={styles.path}>
          <label className={styles.pathLabel}>
            <span className={styles.choiceName}>{label}</span>
            <input
              className={styles.pathInput}
              value={value(key)}
              placeholder={absent}
              onChange={(event) => setDrafts((previous) => ({ ...previous, [key]: event.target.value }))}
            />
          </label>
          <button type="button" className={styles.close} onClick={() => save(key)}>
            save
          </button>
        </div>
      ))}
      <p className={styles.note}>
        Saved under <code>render</code> in the settings file and used by the next render. Empty
        means the placeholder. The two diffusion paths point at a checkout&apos;s{' '}
        <code>tools/sd-filter/sd_filter.py</code> and a venv with torch; the neural switch stays off
        until both are set and torch sees a CUDA device.
      </p>
      {problem !== undefined && (
        <p className={styles.problem} role="alert">
          The setting was not written: {problem}
        </p>
      )}
    </fieldset>
  )
}
