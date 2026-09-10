/**
 * The window: the player's picture, and the state of the thing producing it.
 *
 * This phase is the walking skeleton — a preview, a reading strip and the
 * refusals. The panels that edit a preset arrive on top of it.
 */
import { useCallback, useEffect, useState } from 'react'

import { isKnownPlayerVersion, EXPECTED_PLAYER_VERSION } from '@shared/protocol'

import { Banner } from './components/Banner'
import { Editor } from './views/Editor'
import { Footer } from './components/Footer'
import { Preview, type PreviewStats } from './components/Preview'
import { usePlayerEvents } from './hooks/usePlayerEvents'

import styles from './App.module.css'

interface AppInfo {
  studioVersion: string
  playerPath: string | undefined
  playerSource: string | undefined
}

export function App(): JSX.Element {
  const player = usePlayerEvents()
  const [stats, setStats] = useState<PreviewStats>({ dropped: 0, delivered: 0 })
  const [info, setInfo] = useState<AppInfo>()
  /** The last save's refusal, if it had one; cleared by the next save. */
  const [saveProblem, setSaveProblem] = useState<string>()

  useEffect(() => {
    void window.api.app.getInfo().then(setInfo)
  }, [])

  const onStats = useCallback((next: PreviewStats) => setStats(next), [])

  const unknownVersion = player.hello !== undefined && !isKnownPlayerVersion(player.hello.version)
  const missingPlayer = info !== undefined && info.playerPath === undefined
  const problem = player.problems[0]

  return (
    <div className={styles.app}>
      <header className={styles.header}>
        <h1 className={styles.title}>Ritmolux Studio</h1>
        <span className={styles.preset}>{player.preset?.name ?? 'no preset yet'}</span>
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
          />
        )}
        {saveProblem !== undefined && (
          <Banner kind="error" title="The preset was not written" detail={saveProblem} />
        )}
        <div className={styles.workbench}>
          <Preview stream={player.stream} onStats={onStats} />
          <Editor
            system={player.preset?.system}
            file={player.preset?.file}
            reloads={player.reloads}
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
