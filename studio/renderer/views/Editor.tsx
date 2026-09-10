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
import { presetPath, structuralTables } from '@shared/templates'
import {
  readKeys,
  removeKey,
  setKey,
  setPaletteName,
  setStop,
  type Key,
  type Stop,
} from '@shared/toml'

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
  const { file: state, commit, write } = useActivePreset(file, reloads)
  const [tab, setTab] = useState<Tab>('parameters')

  const text = state.status === 'ready' ? state.text : undefined
  const palette = state.status === 'ready' ? state.palette : undefined

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
    () =>
      palette === undefined
        ? { name: undefined, stops: [] }
        : { ...palette, stops: draft ?? palette.stops },
    [palette, draft],
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

  const writable = state.status === 'ready'

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

      <p className={styles.status}>
        {state.status === 'ready' && <span className={styles.path}>{state.path}</span>}
        {state.status === 'loading' && 'opening the preset file…'}
        {state.status === 'waiting' && 'waiting for the player to name a preset'}
        {state.status === 'embedded' &&
          'this preset is one of the built-in set and has no file — a drag moves the picture, and nothing can be written'}
        {state.status === 'failed' && `could not open ${state.path}: ${state.reason}`}
      </p>

      {tab === 'parameters' && (
        <ParamPanel
          rosters={rostersFor(schema.document, system)}
          bindings={state.status === 'ready' ? state.bindings : []}
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
            onCreate={(fileName, document) => {
              if (dir === null) return
              void window.api.preset
                .write(presetPath(dir, fileName), document)
                .then((result) => onProblem(result.ok ? undefined : result.reason))
            }}
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
