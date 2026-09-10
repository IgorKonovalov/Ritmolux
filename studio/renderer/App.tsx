/**
 * The window: the player's picture, and the state of the thing producing it.
 *
 * This phase is the walking skeleton — a preview, a reading strip and the
 * refusals. The panels that edit a preset arrive on top of it.
 */
import { useCallback, useEffect, useState } from 'react'

import { DEFAULT_PLAYER_MODE, type PlayerMode } from '@shared/player-mode'
import { isKnownPlayerVersion, EXPECTED_PLAYER_VERSION } from '@shared/protocol'

import { Banner } from './components/Banner'
import { Editor } from './views/Editor'
import { Footer } from './components/Footer'
import { Preview, type PreviewStats } from './components/Preview'
import { ProblemsModal } from './components/ProblemsModal'
import { Rotation } from './components/Rotation'
import { Settings } from './views/Settings'
import { useHeldRotation } from './hooks/useHeldRotation'
import { usePlayerActions } from './hooks/usePlayer'
import { usePlayerEvents } from './hooks/usePlayerEvents'

import styles from './App.module.css'

interface AppInfo {
  studioVersion: string
  playerPath: string | undefined
  playerSource: string | undefined
  playerMode: PlayerMode
}

export function App(): JSX.Element {
  const player = usePlayerEvents()
  const actions = usePlayerActions()
  const rotation = useHeldRotation(player.hello, actions.transport)
  const [stats, setStats] = useState<PreviewStats>({ dropped: 0, delivered: 0 })
  const [info, setInfo] = useState<AppInfo>()
  /** The last save's refusal, if it had one; cleared by the next save. */
  const [saveProblem, setSaveProblem] = useState<string>()
  const [settingsOpen, setSettingsOpen] = useState(false)
  const [problemsOpen, setProblemsOpen] = useState(false)

  useEffect(() => {
    void window.api.app.getInfo().then(setInfo)
  }, [])

  const onStats = useCallback((next: PreviewStats) => setStats(next), [])

  const unknownVersion = player.hello !== undefined && !isKnownPlayerVersion(player.hello.version)
  const missingPlayer = info !== undefined && info.playerPath === undefined
  const problem = player.problems[0]
  // The count is the reducer's, not a number this view keeps: how many problems
  // are held, and how many are dropped past the bound, is settled in one place.
  const more = player.problems.length > 1

  return (
    <div className={styles.app}>
      <header className={styles.header}>
        <h1 className={styles.title}>Ritmolux Studio</h1>
        <span className={styles.preset}>{player.preset?.name ?? 'no preset yet'}</span>
        <Rotation held={rotation.held} onResume={rotation.resume} />
        <button
          type="button"
          className={styles.settings}
          aria-expanded={settingsOpen}
          onClick={() => setSettingsOpen((open) => !open)}
        >
          settings
        </button>
      </header>

      <main className={styles.main}>
        {missingPlayer && (
          <Banner
            kind="error"
            title="No player found"
            detail={
              'Looked in the bundle, then the studio settings, then PATH. ' +
              'Set "playerPath" in the studio settings file to a ritmolux build.'
            }
          />
        )}
        {unknownVersion && (
          <Banner
            kind="error"
            title="This player is not one the studio drives"
            detail={`It reports ${player.hello?.version ?? '?'}; the studio was built against ${EXPECTED_PLAYER_VERSION}. The child has been stopped.`}
          />
        )}
        {problem !== undefined && (
          <Banner
            kind={problem.kind}
            title={
              problem.kind === 'error'
                ? 'A preset failed to load'
                : 'A preset loaded with a problem'
            }
            detail={`${problem.file}${problem.line !== null ? `:${problem.line}` : ''} — ${problem.message}`}
            action={
              more && (
                <button
                  type="button"
                  className={styles.problems}
                  onClick={() => setProblemsOpen(true)}
                >
                  {player.problems.length} problems
                </button>
              )
            }
          />
        )}
        {problemsOpen && (
          <ProblemsModal problems={player.problems} onClose={() => setProblemsOpen(false)} />
        )}
        {saveProblem !== undefined && (
          <Banner kind="error" title="The preset was not written" detail={saveProblem} />
        )}
        {settingsOpen && (
          // Above the workbench rather than over it: unmounting the preview to
          // show a settings dialog would drop the frame port's listener and
          // the picture with it.
          <Settings
            running={info?.playerMode ?? DEFAULT_PLAYER_MODE}
            playerPath={info?.playerPath}
            playerSource={info?.playerSource}
            studioVersion={info?.studioVersion ?? '—'}
            onClose={() => setSettingsOpen(false)}
          />
        )}
        <div className={styles.workbench}>
          <Preview stream={player.stream} onStats={onStats} />
          <Editor
            system={player.preset?.system}
            file={player.preset?.file}
            reloads={player.reloads}
            problems={player.problems}
            roster={player.roster}
            active={player.preset?.name}
            dir={player.dir}
            onProblem={setSaveProblem}
          />
        </div>
      </main>

      <Footer
        hello={player.hello}
        health={player.health}
        stats={stats}
        streamLabel={
          player.stream === undefined
            ? undefined
            : `${player.stream.width}x${player.stream.height} @ ${player.stream.fps} ${player.stream.format}`
        }
      />
    </div>
  )
}
