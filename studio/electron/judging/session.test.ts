/**
 * A judging session copies its set, folds the player's marks into verdicts,
 * appends them to the ledger, writes edits back only over unmoved sources, and
 * deletes only its own directories (Plan 0254 Phase 2).
 */
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

import { describe, expect, it } from 'vitest'

import type { LedgerLine } from '@shared/judging'

import { JudgingService, runName, runStarted, type PlayerLaunch } from './session'

const preset = (name: string): string => `# a header comment\nname = "${name}"\nscene = "x"\n\n[params]\na = 1\n`

function harness(files: Record<string, string>): {
  source: string
  root: string
  service: JudgingService
  launches: (PlayerLaunch | undefined)[]
} {
  const base = mkdtempSync(join(tmpdir(), 'rlx-judging-'))
  const source = join(base, 'presets')
  const root = join(base, 'userData', 'judging')
  mkdirSync(source, { recursive: true })
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(join(source, path, '..'), { recursive: true })
    writeFileSync(join(source, path), text)
  }
  const launches: (PlayerLaunch | undefined)[] = []
  let tick = 0
  const service = new JudgingService({
    root,
    sourceDir: () => source,
    restartPlayer: (launch) => launches.push(launch),
    // A second apart, so every session in a test gets its own run name.
    now: () => new Date(Date.UTC(2026, 9, 7, 12, 0, tick++)),
  })
  return { source, root, service, launches }
}

const SET = {
  'attractor_clifford.toml': preset('Clifford'),
  'attractor_dejong.toml': preset('De Jong'),
  'attractor_lorenz.toml': preset('Lorenz'),
  'curve_rose.toml': preset('Rose'),
  'proposed/attractor_draft.toml': preset('Draft'),
}

function ledgerOf(root: string): LedgerLine[] {
  const file = join(root, 'ledger.jsonl')
  return readFileSync(file, 'utf8')
    .split('\n')
    .filter((line) => line !== '')
    .map((line) => JSON.parse(line) as LedgerLine)
}

describe('run names', () => {
  it('are the basic ISO form of the start and read back to the extended one', () => {
    const run = runName(new Date(Date.UTC(2026, 9, 7, 12, 30, 5, 250)))
    expect(run).toBe('20261007T123005Z')
    expect(runStarted(run)).toBe('2026-10-07T12:30:05Z')
    expect(runStarted('not-a-run')).toBeNull()
  })
})

describe('JudgingService', () => {
  it('copies exactly the files a family prefix draws and restarts the player on them', () => {
    const h = harness(SET)
    const started = h.service.start({ kind: 'family', prefix: 'attractor' })
    expect(started.ok).toBe(true)
    if (!started.ok) return
    expect(started.value.set).toBe('family:attractor')
    expect(started.value.presets.map((p) => p.stem)).toEqual([
      'attractor_clifford',
      'attractor_dejong',
      'attractor_lorenz',
    ])

    const dir = join(h.root, 'sessions', started.value.run)
    // The draft in `proposed/` and the other family stay out.
    expect(readdirSync(join(dir, 'presets')).sort()).toEqual([
      'attractor_clifford.toml',
      'attractor_dejong.toml',
      'attractor_lorenz.toml',
    ])
    expect(readFileSync(join(dir, 'presets', 'attractor_clifford.toml'), 'utf8')).toBe(
      preset('Clifford'),
    )
    const sources = JSON.parse(readFileSync(join(dir, 'sources.json'), 'utf8')) as {
      files: { stem: string; source: string; sha256: string }[]
    }
    expect(sources.files.map((f) => f.source)).toEqual([
      join(h.source, 'attractor_clifford.toml'),
      join(h.source, 'attractor_dejong.toml'),
      join(h.source, 'attractor_lorenz.toml'),
    ])
    expect(sources.files[0].sha256).toMatch(/^[0-9a-f]{64}$/)

    expect(h.launches).toEqual([
      { presetDir: join(dir, 'presets'), marks: join(dir, 'marks.toml') },
    ])
    // The renderer is told the directory the player runs on, which is what lets
    // the editor write a copy in place.
    expect(started.value.presetDir).toBe(join(dir, 'presets'))
    // The player is the marks file's only writer; the session makes none.
    expect(existsSync(join(dir, 'marks.toml'))).toBe(false)
  })

  it('draws `proposed/` and an explicit list', () => {
    const h = harness(SET)
    const drafts = h.service.start({ kind: 'proposed' })
    expect(drafts.ok && drafts.value.presets.map((p) => p.stem)).toEqual(['attractor_draft'])
    h.service.end()
    const list = h.service.start({ kind: 'list', files: ['curve_rose.toml', 'proposed/attractor_draft.toml'] })
    expect(list.ok && list.value.set).toBe('list:2 files')
    expect(list.ok && list.value.presets.map((p) => p.stem)).toEqual(['curve_rose', 'attractor_draft'])
  })

  it('refuses a listed path outside the source directory', () => {
    const h = harness(SET)
    const result = h.service.start({ kind: 'list', files: ['../elsewhere.toml'] })
    expect(result.ok).toBe(false)
    expect(h.launches).toEqual([])
  })

  it('refuses a set in which two files share a display name, naming both', () => {
    const h = harness({ 'a_one.toml': preset('Same'), 'a_two.toml': preset('Same') })
    const result = h.service.start({ kind: 'family', prefix: 'a' })
    expect(result.ok).toBe(false)
    if (result.ok) return
    expect(result.reason).toContain('a_one')
    expect(result.reason).toContain('a_two')
    expect(h.launches).toEqual([])
  })

  it('folds favourite as keep, hidden as cut, neither as tune, favourite winning over hidden', () => {
    const h = harness(SET)
    h.service.start({ kind: 'family', prefix: 'attractor' })
    h.service.marks({ favourite: ['Clifford'], hidden: ['Clifford', 'Lorenz'] })
    expect(h.service.counts).toEqual({ keep: 1, cut: 1, tune: 1 })

    const ended = h.service.end()
    expect(ended.ok).toBe(true)
    const verdicts = Object.fromEntries(ledgerOf(h.root).map((line) => [line.stem, line.verdict]))
    expect(verdicts).toEqual({
      attractor_clifford: 'keep',
      attractor_dejong: 'tune',
      attractor_lorenz: 'cut',
    })
    // Back on the normal vector.
    expect(h.launches.at(-1)).toBeUndefined()
  })

  it('appends one line per readable preset and rewrites no earlier line', () => {
    const h = harness({ ...SET, 'attractor_nameless.toml': 'scene = "x"\n' })
    h.service.start({ kind: 'family', prefix: 'attractor' })
    h.service.end()
    const first = readFileSync(join(h.root, 'ledger.jsonl'), 'utf8')
    // The file with no name is listed but gets no verdict.
    expect(first.trim().split('\n')).toHaveLength(3)
    expect(h.service.session).toBeNull()

    h.service.start({ kind: 'family', prefix: 'curve' })
    h.service.marks({ favourite: [], hidden: ['Rose'] })
    h.service.end()
    const second = readFileSync(join(h.root, 'ledger.jsonl'), 'utf8')
    expect(second.startsWith(first)).toBe(true)
    const added = ledgerOf(h.root).slice(3)
    expect(added).toEqual([
      {
        v: 1,
        run: '20261007T120001Z',
        set: 'family:curve',
        source: h.source,
        stem: 'curve_rose',
        name: 'Rose',
        verdict: 'cut',
      },
    ])
  })

  it('writes nothing to the ledger for a session that never ended', () => {
    const h = harness(SET)
    h.service.start({ kind: 'family', prefix: 'attractor' })
    h.service.marks({ favourite: ['Clifford'], hidden: [] })
    expect(existsSync(join(h.root, 'ledger.jsonl'))).toBe(false)
  })

  it('reports the edited copies and writes back one whose source did not move', async () => {
    const h = harness(SET)
    const started = h.service.start({ kind: 'family', prefix: 'attractor' })
    if (!started.ok) throw new Error(started.reason)
    const copies = join(h.root, 'sessions', started.value.run, 'presets')
    writeFileSync(join(copies, 'attractor_clifford.toml'), preset('Clifford').replace('a = 1', 'a = 2'))
    writeFileSync(join(copies, 'attractor_lorenz.toml'), preset('Lorenz').replace('a = 1', 'a = 3'))
    // The author edits the source of one of them outside the studio meanwhile.
    writeFileSync(join(h.source, 'attractor_lorenz.toml'), preset('Lorenz').replace('a = 1', 'a = 9'))

    const ended = h.service.end()
    expect(ended.ok && ended.value.edited).toEqual(['attractor_clifford', 'attractor_lorenz'])

    const back = await h.service.writeBack(['attractor_clifford', 'attractor_lorenz'])
    expect(back.ok).toBe(true)
    if (!back.ok) return
    expect(back.value.written).toEqual(['attractor_clifford'])
    expect(back.value.refused.map((r) => r.stem)).toEqual(['attractor_lorenz'])
    expect(readFileSync(join(h.source, 'attractor_clifford.toml'), 'utf8')).toContain('a = 2')
    // The moved source keeps its own edit; nothing is merged.
    expect(readFileSync(join(h.source, 'attractor_lorenz.toml'), 'utf8')).toContain('a = 9')
  })

  it('removes the chosen ended sessions and refuses the running one and a path outside the root', () => {
    const h = harness(SET)
    h.service.start({ kind: 'family', prefix: 'attractor' })
    h.service.end()
    h.service.start({ kind: 'family', prefix: 'curve' })
    h.service.end()
    const running = h.service.start({ kind: 'proposed' })
    if (!running.ok) throw new Error(running.reason)
    expect(h.service.listSessions().map((s) => s.run)).toEqual([
      '20261007T120002Z',
      '20261007T120001Z',
      '20261007T120000Z',
    ])
    expect(h.service.listSessions()[0].started).toBe('2026-10-07T12:00:02Z')

    const result = h.service.deleteSessions([
      '20261007T120000Z',
      running.value.run,
      '..',
      '../../presets',
      join(h.source),
    ])
    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(result.value.deleted).toEqual(['20261007T120000Z'])
    expect(result.value.refused.map((r) => r.run)).toEqual([
      running.value.run,
      '..',
      '../../presets',
      h.source,
    ])
    expect(h.service.listSessions().map((s) => s.run)).toEqual([
      '20261007T120002Z',
      '20261007T120001Z',
    ])
    // The source directory is untouched.
    expect(readdirSync(h.source)).toContain('attractor_clifford.toml')
  })
})
