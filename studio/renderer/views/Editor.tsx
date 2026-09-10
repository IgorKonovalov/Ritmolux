/**
 * The editing surface: the preset on screen, and the three ways to move it.
 *
 * Everything it needs is a fact the player reported — which system to build the
 * panel for, and which file to write. Nothing here resolves a path, guesses a
 * system, or renders a row the engine did not declare.
 *
 * The three tabs differ in how fast they reach the picture, which is why they
 * are separate rather than one scroll. A parameter drag is live: the override
 * arrives on the player's next frame. A palette edit and a file save are not:
 * they go to disk, and the picture follows when the watcher's poll brings the
 * reload. Presenting them as one surface would suggest the second kind is
 * broken for the ~150 ms it is not.
 */
import { useCallback, useEffect, useMemo, useState } from 'react'

import { mapElementEditor } from '@shared/fields'
import { rostersFor, type SchemaDocument, type TableKey } from '@shared/schema'
import { canTemplate, structuralTables, templateFor } from '@shared/templates'
import {
  readKeys,
  removeKey,
  setKey,
  setPaletteName,
  setStop,
  type Key,
  type Stop,
} from '@shared/toml'

import { ForkPrompt } from '../components/ForkPrompt'
import { MapEditor } from '../components/MapEditor'
import { PaletteEditor } from '../components/PaletteEditor'
import { ParamPanel } from '../components/ParamPanel'
import { PresetEditor } from '../components/PresetEditor'
import { SystemPicker } from '../components/SystemPicker'
import { TableEditor } from '../components/TableEditor'
import { Library } from './Library'
import { useRoster } from '../hooks/useRoster'
import { useActivePreset } from '../hooks/useActivePreset'
import { usePlayerActions } from '../hooks/usePlayer'
import { useSchema } from '../hooks/useSchema'
import type { Problem } from '../hooks/usePlayerEvents'

import styles from './Editor.module.css'

export interface EditorProps {
  system: string | undefined
  /** The player's own path for the preset on screen; `null` for the embedded set. */
  file: string | null | undefined
  /** Rises on every reload, so the file is re-read after a save. */
  reloads: number
  problems: Problem[]
  /** The preset names the player last reported, in roster order. */
  roster: string[]
  /** The name on screen, so the library can mark it. */
  active: string | undefined
  /** Where the watcher is looking, or `null` when nothing resolved. */
  dir: string | null
  onProblem: (reason: string | undefined) => void
}

const TABS = ['parameters', 'structure', 'palette', 'file', 'library'] as const
type Tab = (typeof TABS)[number]

/**
 * The document root's own map keys — `[hold]`, `[smoothing]` — which have no
 * `TableSpec` of their own because in the file they are sections at the top
 * level rather than tables a preset writes under a header of the schema's
 * naming.
 *
 * `structuralTables` drops the root for a good reason: its `system` has the
 * picker, its `params` has the parameter panel, and its table-valued keys are
 * each rendered as their own section. What is left over is exactly the maps,
 * and until this they had nowhere to be.
 */
function rootMaps(document: SchemaDocument): TableKey[] {
  const root = document.tables.find((table) => table.name === 'preset')
  return (root?.keys ?? []).filter((key) => mapElementEditor(key) !== undefined)
}

export function Editor({
  system,
  file,
  reloads,
  problems,
  roster,
  active,
  dir,
  onProblem,
}: EditorProps): JSX.Element {
  const schema = useSchema()
  const actions = usePlayerActions()
  const library = useRoster(roster, active, actions.selectPreset)
  /**
   * What an embedded preset forks from.
   *
   * It is a template for the system on screen, not a copy of what is drawn: no
   * event carries the embedded document's own text and the studio resolves
   * nothing of its own (ADR-0184), so the honest first file is the schema's
   * defaults for that system plus whatever the gesture changed.
   */
  const base = useMemo(
    () =>
      file === null &&
      system !== undefined &&
      schema.status === 'ready' &&
      canTemplate(schema.document, system)
        ? templateFor(schema.document, { name: active ?? system, system })
        : undefined,
    [file, system, schema, active],
  )
  const preset = useActivePreset(file, reloads, { dir, name: active, base })
  const { file: state, text, commit, write } = preset
  const [tab, setTab] = useState<Tab>('parameters')

  /**
   * A fork the player has not seen yet.
   *
   * `ctl/preset` naming a preset the roster does not hold changes nothing
   * (spec 0003), so the switch waits for the watcher's poll to bring the new
   * file in rather than firing into the gap between the write and the reload.
   */
  const [awaiting, setAwaiting] = useState<string>()
  const select = library.select
  useEffect(() => {
    if (awaiting === undefined || !roster.includes(awaiting)) return
    select(awaiting)
    setAwaiting(undefined)
  }, [awaiting, roster, select])

  /**
   * The stops as the ramp shows them mid-drag.
   *
   * A palette stop has no control action — it is not a bindable parameter — so
   * the picture cannot follow a drag and only the ramp can. The draft is
   * dropped whenever the file changes underneath, so what is shown after a
   * reload is the file rather than a gesture the disk never took.
   */
  const [draft, setDraft] = useState<Stop[]>()
  useEffect(() => setDraft(undefined), [text])

  const shownPalette = useMemo(
    () => ({ ...preset.palette, stops: draft ?? preset.palette.stops }),
    [preset.palette, draft],
  )

  const onDrag = useCallback(
    (name: string, value: number) => actions.setParam(name, value),
    [actions],
  )
  const onCommit = useCallback(
    (name: string, value: number) => void commit(name, value).then(onProblem),
    [commit, onProblem],
  )

  /** Build the next document with the shared editors, then write it whole. */
  const edit = useCallback(
    (next: (current: string) => string) => {
      if (text === undefined) return
      let document: string
      try {
        document = next(text)
      } catch (error) {
        onProblem((error as Error).message)
        return
      }
      void write(document).then(onProblem)
    },
    [text, write, onProblem],
  )

  const readSection = useCallback(
    (section: string): Key[] => (text === undefined ? [] : readKeys(text, section)),
    [text],
  )
  const onSetKey = useCallback(
    (section: string, key: string, literal: string) =>
      edit((current) => setKey(current, section, key, literal)),
    [edit],
  )
  const onRemoveKey = useCallback(
    (section: string, key: string) => edit((current) => removeKey(current, section, key)),
    [edit],
  )

  /**
   * Every parameter the active preset may bind, offered as the names a map's
   * add row suggests. The same roster the parameter panel is built from, so a
   * `[hold]` entry is offered exactly the names a hold can reach.
   */
  const bindable = useMemo(
    () =>
      schema.status === 'ready'
        ? rostersFor(schema.document, system).flatMap((roster) =>
            roster.params.map((spec) => spec.name),
          )
        : [],
    [schema, system],
  )

  if (schema.status === 'loading') {
    return <p className={styles.note}>Reading the engine&apos;s parameter schema…</p>
  }
  if (schema.status === 'failed') {
    return (
      <p className={styles.note}>
        No panel: {schema.reason}. Every row here is generated from what the player declares, so
        there is nothing to show without it.
      </p>
    )
  }

  const writable = preset.writable

  return (
    <div className={styles.editor}>
      <div className={styles.tabs} role="tablist">
        {TABS.map((name) => (
          <button
            key={name}
            type="button"
            role="tab"
            aria-selected={tab === name}
            className={tab === name ? styles.tabOn : styles.tab}
            onClick={() => setTab(name)}
          >
            {name}
          </button>
        ))}
      </div>

      {preset.fork === undefined ? (
        <p className={styles.status}>
          {state.status === 'ready' && <span className={styles.path}>{state.path}</span>}
          {state.status === 'loading' && 'opening the preset file…'}
          {state.status === 'waiting' && 'waiting for the player to name a preset'}
          {state.status === 'embedded' &&
            'this preset is one of the built-in set and has no file — an edit saves a copy, which becomes its first one'}
          {state.status === 'failed' && `could not open ${state.path}: ${state.reason}`}
        </p>
      ) : (
        <ForkPrompt
          // Keyed by what is being forked, so a gesture against a different
          // preset re-seeds the field rather than keeping the name typed for the
          // previous one.
          key={preset.fork.source}
          request={preset.fork}
          onSave={(chosen) =>
            void preset.keep(chosen).then((reason) => {
              onProblem(reason)
              if (reason === undefined) setAwaiting(chosen)
            })
          }
          onCancel={preset.discard}
        />
      )}

      {tab === 'parameters' && (
        <ParamPanel
          rosters={rostersFor(schema.document, system)}
          bindings={preset.bindings}
          writable={writable}
          onDrag={onDrag}
          onCommit={onCommit}
        />
      )}

      {tab === 'structure' && schema.status === 'ready' && (
        <div className={styles.pane}>
          <SystemPicker
            document={schema.document}
            current={system}
            writable={writable}
            // The root, not a `[preset]` table: the schema calls the document's own
            // keys `preset`, and in the file they sit before the first header.
            onChange={(next) => edit((current) => setKey(current, '', 'system', `"${next}"`))}
          />
          {rootMaps(schema.document).map((key) => {
            const control = mapElementEditor(key)
            if (control === undefined) return null
            return (
              <MapEditor
                key={key.name}
                // A root map is a top-level section: `[hold]`, not `[preset.hold]`.
                section={key.name}
                spec={key}
                control={control}
                entries={readSection(key.name)}
                suggestions={bindable}
                writable={writable}
                onSet={onSetKey}
                onRemove={onRemoveKey}
              />
            )
          })}
          {structuralTables(schema.document).map((table) => (
            <TableEditor
              key={table.name}
              table={table}
              keys={readSection(table.name)}
              readSection={readSection}
              suggestions={bindable}
              writable={writable}
              onSet={onSetKey}
              onRemove={onRemoveKey}
            />
          ))}
        </div>
      )}

      {tab === 'library' && (
        <div className={styles.pane}>
          <Library
            document={schema.document}
            roster={library.names}
            active={library.pending ?? library.active}
            dir={dir}
            system={system}
            onSelect={library.select}
            // Through the same create the fork takes: it refuses a name that is
            // already a preset, and it remembers the file as this session's own so
            // editing what was just made does not ask for a name again.
            onCreate={(fileName, document) =>
              void preset.create(fileName, document).then(onProblem)
            }
            onProblem={onProblem}
          />
        </div>
      )}

      {tab === 'palette' && (
        <div className={styles.pane}>
          <PaletteEditor
            document={schema.document}
            palette={shownPalette}
            writable={writable}
            onName={(name) => edit((current) => setPaletteName(current, name))}
            onMove={(index, at) =>
              setDraft((previous) =>
                (previous ?? shownPalette.stops).map((stop, i) =>
                  i === index ? { ...stop, at } : stop,
                ),
              )
            }
            onCommit={(index, at) => edit((current) => setStop(current, index, { at }))}
            onRecolour={(index, color) => edit((current) => setStop(current, index, { color }))}
          />
        </div>
      )}

      {tab === 'file' && (
        <div className={styles.pane}>
          <PresetEditor
            path={state.status === 'ready' ? state.path : undefined}
            text={text}
            grammar={schema.document.grammar}
            problems={problems}
            onSave={(next) => edit(() => next)}
          />
        </div>
      )}
    </div>
  )
}
