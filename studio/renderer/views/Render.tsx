/**
 * The Render view: one track, one preset, and an MP4 at the end (ADR-0262).
 *
 * The view only describes the job. Main transcodes the track, counts its bars
 * with the player's `--bars`, and runs the player's `--render` into `ffmpeg`;
 * what arrives here is the grid, the encoder's frame count and how the job
 * ended. Kept mounted while hidden, because a render outlives the panel being
 * open and its progress would otherwise have nowhere to land.
 *
 * The prompts are held as `[{at_bar, prompt}]`, ascending and unique, which is
 * the document `sd_filter.py --timeline` reads.
 *
 * Start leaves `<output>.render.json` beside the MP4, and **open job** restores
 * every field from one. A job whose track has moved restores the rest and asks
 * for the track, and the next pick replaces only that.
 */
import { useEffect, useState } from 'react'

import {
  DEFAULT_RENDER_FPS,
  DEFAULT_RENDER_SIZE,
  DEFAULT_RENDER_TIER,
  DIFFUSION_PROFILES,
  etaSeconds,
  formatDuration,
  fpsSchema,
  RENDER_TIERS,
  sizeSchema,
  type DiffusionProfile,
  type PreparedTrack,
  type ProbeResult,
  type RenderEvent,
  type RenderTier,
  type SidecarPace,
} from '@shared/render'
import { timelineProblem, type TimelineEntry } from '@shared/timeline'

import { BarStrip } from '../components/BarStrip'

import styles from './Render.module.css'

/** A new track starts with one prompt on bar 1, waiting for its text. */
const FIRST_PROMPT: TimelineEntry[] = [{ at_bar: 1, prompt: '' }]

export interface RenderProps {
  /** Every preset the player's library holds, in roster order. */
  roster: string[]
  /** The preset on screen, which the picker starts on. */
  active: string | undefined
  hidden: boolean
  onClose: () => void
}

/** Where the job is, as the view shows it. */
type JobState =
  | { kind: 'idle' }
  | { kind: 'starting' }
  | {
      kind: 'running'
      output: string
      frame: number
      frames: number
      elapsedMs: number
      sidecar?: SidecarPace
    }
  | { kind: 'done'; output: string }
  | { kind: 'cancelled' }
  | Extract<RenderEvent, { kind: 'failed' }>

function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path
}

export function Render({ roster, active, hidden, onClose }: RenderProps): JSX.Element {
  const [source, setSource] = useState<string>()
  const [track, setTrack] = useState<PreparedTrack>()
  const [preparing, setPreparing] = useState(false)
  const [picked, setPicked] = useState<string>()
  const [fps, setFps] = useState(DEFAULT_RENDER_FPS)
  const [size, setSize] = useState(DEFAULT_RENDER_SIZE)
  const [tier, setTier] = useState<RenderTier>(DEFAULT_RENDER_TIER)
  const [chosenOutput, setChosenOutput] = useState<string>()
  const [suggested, setSuggested] = useState<string>()
  const [problem, setProblem] = useState<string>()
  const [job, setJob] = useState<JobState>({ kind: 'idle' })
  /**
   * The prompts, by bar number. A new rate re-counts the bars and keeps these
   * numbers; a prompt the new grid ends before is flagged, not moved.
   */
  const [timeline, setTimeline] = useState<TimelineEntry[]>(FIRST_PROMPT)
  /** The sidecar probe's answer, asked once the view is first opened. */
  const [probe, setProbe] = useState<ProbeResult>()
  const [neural, setNeural] = useState(false)
  const [profile, setProfile] = useState<DiffusionProfile>('fast')
  const [negative, setNegative] = useState('')
  const [seed, setSeed] = useState('')
  /** A job was opened whose track has moved; the next pick only replaces the track. */
  const [relinking, setRelinking] = useState(false)

  const preset = picked ?? active ?? roster[0]
  const neuralOn = neural && probe?.ready === true
  const seedValid = seed.trim() === '' || /^\d+$/.test(seed.trim())
  const output = chosenOutput ?? suggested
  const fpsValid = fpsSchema.safeParse(fps).success
  const sizeValid = sizeSchema.safeParse(size).success
  const running = job.kind === 'running' || job.kind === 'starting'

  useEffect(
    () =>
      window.api.render.onEvent((event) => {
        switch (event.kind) {
          case 'started':
            setJob({ kind: 'running', output: event.output, frame: 0, frames: event.frames, elapsedMs: 0 })
            break
          case 'progress':
            setJob((previous) =>
              previous.kind === 'running'
                ? {
                    ...previous,
                    frame: event.frame,
                    frames: event.frames,
                    elapsedMs: event.elapsedMs,
                    sidecar: event.sidecar,
                  }
                : previous,
            )
            break
          default:
            setJob(event)
        }
      }),
    [],
  )

  // Asked when the view first opens, not at launch: the probe starts an
  // interpreter that imports torch, which costs seconds nobody rendering pays.
  const probed = probe !== undefined
  useEffect(() => {
    if (hidden || probed) return
    void window.api.render.probe(false).then(setProbe)
  }, [hidden, probed])

  // The grid is counted at a rate, so a new rate is a new `--bars`.
  useEffect(() => {
    if (source === undefined || !fpsValid) return
    let current = true
    setPreparing(true)
    void window.api.render.prepare(source, fps).then((result) => {
      if (!current) return
      setPreparing(false)
      if (result.ok) {
        setTrack(result.value)
        setProblem(undefined)
      } else {
        setTrack(undefined)
        setProblem(result.reason)
      }
    })
    return () => {
      current = false
    }
  }, [source, fps, fpsValid])

  useEffect(() => {
    if (source === undefined || preset === undefined) return
    let current = true
    void window.api.render.suggestOutput(source, preset).then((result) => {
      if (current && result.ok) setSuggested(result.value)
    })
    return () => {
      current = false
    }
  }, [source, preset])

  const pickAudio = (): void => {
    void window.api.render.pickAudio().then((path) => {
      if (path === null) return
      setTrack(undefined)
      // A job whose track moved keeps everything but the track; any other pick
      // is a new render and starts over.
      if (relinking) {
        setRelinking(false)
        setProblem(undefined)
      } else {
        setChosenOutput(undefined)
        setTimeline(FIRST_PROMPT)
      }
      setSource(path)
    })
  }

  /** Restore the whole view from a `<output>.render.json`. */
  const openJob = (): void => {
    void window.api.render.openJob().then((result) => {
      if (result === null) return
      if (!result.ok) {
        setProblem(`The job was not opened: ${result.reason}`)
        return
      }
      const { request, sourceMissing } = result.value
      setPicked(request.preset)
      setFps(request.fps)
      setSize(request.size)
      setTier(request.tier)
      setChosenOutput(request.output)
      setNeural(request.neural !== null)
      setProfile(request.neural?.profile ?? 'fast')
      setNegative(request.neural?.negative ?? '')
      setSeed(request.neural?.seed === null || request.neural === null ? '' : String(request.neural.seed))
      setTimeline(request.neural?.timeline ?? FIRST_PROMPT)
      setTrack(undefined)
      setJob({ kind: 'idle' })
      if (sourceMissing) {
        setSource(undefined)
        setRelinking(true)
        setProblem(
          `The job's track is no longer at ${request.source}. Choose it again; the prompts and settings are kept.`,
        )
      } else {
        setRelinking(false)
        setProblem(undefined)
        setSource(request.source)
      }
    })
  }

  const pickOutput = (): void => {
    void window.api.render.pickOutput(output).then((path) => {
      if (path !== null) setChosenOutput(path)
    })
  }

  const start = (): void => {
    if (source === undefined || preset === undefined || output === undefined) return
    setProblem(undefined)
    setJob({ kind: 'starting' })
    const request = {
      source,
      preset,
      fps,
      size,
      tier,
      output,
      neural: neuralOn
        ? {
            profile,
            negative: negative.trim() === '' ? null : negative.trim(),
            seed: seed.trim() === '' ? null : Number(seed.trim()),
            timeline,
          }
        : null,
    }
    void window.api.render.start(request).then((result) => {
      if (result.ok) return
      setJob({ kind: 'idle' })
      setProblem(result.reason)
    })
  }

  const timelineIssue =
    track === undefined ? undefined : timelineProblem(timeline, track.grid.bar_starts.length)
  // A neural job with no usable prompt is refused here, and again in main
  // before anything is spawned.
  const ready =
    track !== undefined &&
    preset !== undefined &&
    output !== undefined &&
    fpsValid &&
    sizeValid &&
    (!neuralOn || (timelineIssue === undefined && seedValid))

  return (
    <section className={styles.panel} aria-label="Render a clip" hidden={hidden}>
      <div className={styles.bar}>
        <h2 className={styles.heading}>Render</h2>
        <div className={styles.row}>
          <button type="button" className={styles.quiet} onClick={openJob} disabled={running}>
            open job…
          </button>
          <button type="button" className={styles.quiet} onClick={onClose}>
            close
          </button>
        </div>
      </div>

      <div className={styles.row}>
        <button type="button" className={styles.button} onClick={pickAudio} disabled={running}>
          choose track…
        </button>
        <span className={styles.path}>{source ?? 'no track chosen'}</span>
      </div>
      {preparing && <p className={styles.note}>Reading the track and counting its bars…</p>}
      {track !== undefined && (
        <>
          <BarStrip
            grid={track.grid}
            peaks={track.peaks}
            timeline={timeline}
            onChange={setTimeline}
            disabled={running || preparing}
          />
          {timelineIssue !== undefined && <p className={styles.note}>Prompts: {timelineIssue}</p>}
        </>
      )}

      <div className={styles.fields}>
        <label className={styles.field}>
          <span>preset</span>
          <select
            value={preset ?? ''}
            onChange={(event) => setPicked(event.target.value)}
            disabled={running || roster.length === 0}
          >
            {roster.map((name) => (
              <option key={name} value={name}>
                {name}
              </option>
            ))}
          </select>
        </label>
        <label className={styles.field}>
          <span>fps</span>
          <input
            value={fps}
            onChange={(event) => setFps(event.target.value)}
            aria-invalid={!fpsValid}
            disabled={running}
          />
        </label>
        <label className={styles.field}>
          <span>size</span>
          <input
            value={size}
            onChange={(event) => setSize(event.target.value)}
            aria-invalid={!sizeValid}
            disabled={running}
          />
        </label>
        <label className={styles.field}>
          <span>tier</span>
          <select
            value={tier}
            onChange={(event) => setTier(event.target.value as RenderTier)}
            disabled={running}
          >
            {RENDER_TIERS.map((name) => (
              <option key={name} value={name}>
                {name}
              </option>
            ))}
          </select>
        </label>
      </div>

      <fieldset className={styles.neural}>
        <legend className={styles.legend}>
          <label className={styles.switch}>
            <input
              type="checkbox"
              checked={neuralOn}
              disabled={running || probe?.ready !== true}
              onChange={(event) => setNeural(event.target.checked)}
            />
            neural
          </label>
        </legend>
        <div className={styles.row}>
          <span className={probe?.ready === false ? styles.problem : styles.note}>
            {probe === undefined
              ? 'Checking whether the diffusion sidecar can run…'
              : probe.ready
                ? 'The diffusion sidecar is ready: torch sees a CUDA device.'
                : probe.reason}
          </span>
          <button
            type="button"
            className={styles.quiet}
            disabled={running}
            onClick={() => {
              setProbe(undefined)
              void window.api.render.probe(true).then(setProbe)
            }}
          >
            re-check
          </button>
        </div>
        {neuralOn && (
          <div className={styles.fields}>
            <label className={styles.field}>
              <span>profile</span>
              <select
                value={profile}
                onChange={(event) => setProfile(event.target.value as DiffusionProfile)}
                disabled={running}
              >
                {DIFFUSION_PROFILES.map((name) => (
                  <option key={name} value={name}>
                    {name}
                  </option>
                ))}
              </select>
            </label>
            <label className={styles.field}>
              <span>negative prompt</span>
              <input
                value={negative}
                placeholder="the sidecar's default"
                onChange={(event) => setNegative(event.target.value)}
                disabled={running}
              />
            </label>
            <label className={styles.field}>
              <span>seed</span>
              <input
                value={seed}
                placeholder="the sidecar's default"
                aria-invalid={!seedValid}
                onChange={(event) => setSeed(event.target.value)}
                disabled={running}
              />
            </label>
          </div>
        )}
      </fieldset>

      <div className={styles.row}>
        <button
          type="button"
          className={styles.button}
          onClick={pickOutput}
          disabled={running || source === undefined}
        >
          output…
        </button>
        <span className={styles.path}>{output ?? 'chosen once a track is'}</span>
      </div>

      <div className={styles.row}>
        {running ? (
          <button
            type="button"
            className={styles.button}
            onClick={() => void window.api.render.cancel()}
            disabled={job.kind === 'starting'}
          >
            cancel
          </button>
        ) : (
          <button type="button" className={styles.primary} onClick={start} disabled={!ready}>
            start
          </button>
        )}
      </div>

      <JobLine job={job} />
      {problem !== undefined && (
        <p className={styles.problem} role="alert">
          {problem}
        </p>
      )}
    </section>
  )
}

function JobLine({ job }: { job: JobState }): JSX.Element | null {
  switch (job.kind) {
    case 'idle':
      return null
    case 'starting':
      return <p className={styles.note}>Starting…</p>
    case 'running': {
      const eta = etaSeconds(job.frame, job.frames, job.elapsedMs)
      return (
        <div className={styles.progress}>
          <progress value={job.frame} max={job.frames} aria-label="render progress" />
          <span className={styles.note}>
            {job.frame} / {job.frames} frames encoded
            {eta === undefined ? '' : ` · ${formatDuration(eta)} left`} · {fileName(job.output)}
          </span>
          {job.sidecar !== undefined && (
            <span className={styles.note}>
              sidecar: {job.sidecar.frames} frames
              {job.sidecar.secondsPerFrame === undefined
                ? ''
                : ` · ${job.sidecar.secondsPerFrame.toFixed(2)} s per frame`}
            </span>
          )}
        </div>
      )
    }
    case 'done':
      return <p className={styles.note}>Written: {job.output}</p>
    case 'cancelled':
      return <p className={styles.note}>Cancelled; the unfinished file was removed.</p>
    case 'failed':
      return (
        <div className={styles.failure} role="alert">
          <p className={styles.problem}>
            The {job.stage} failed: {job.reason}
          </p>
          {job.tail.length > 0 && <pre className={styles.tail}>{job.tail.join('\n')}</pre>}
          {job.log !== undefined && <p className={styles.note}>Every stage&apos;s output: {job.log}</p>}
        </div>
      )
  }
}
