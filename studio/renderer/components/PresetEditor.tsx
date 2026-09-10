/**
 * The preset file, editable, with the player's own complaints marked on it.
 *
 * One CodeMirror view for the component's life. React owns *when* the document
 * is replaced and CodeMirror owns the document itself, so the two do not fight
 * over the cursor: a reload replaces the text only when the author has no
 * unsaved edit, because taking a half-typed expression away mid-keystroke is
 * worse than showing a file one save behind.
 */
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands'
import { syntaxHighlighting, defaultHighlightStyle } from '@codemirror/language'
import { lintGutter, setDiagnostics, type Diagnostic } from '@codemirror/lint'
import { EditorState, type Extension } from '@codemirror/state'
import { EditorView, keymap, lineNumbers } from '@codemirror/view'
import { useEffect, useRef, useState } from 'react'

import { markersFor, type PlayerProblem } from '../editor/diagnostics'
import { presetLanguage } from '../editor/expr-language'

import styles from './PresetEditor.module.css'

export interface PresetEditorProps {
  path: string | undefined
  /** The file as the player last loaded it. */
  text: string | undefined
  problems: PlayerProblem[]
  onSave: (text: string) => void
}

const BASE: Extension[] = [
  lineNumbers(),
  history(),
  lintGutter(),
  syntaxHighlighting(defaultHighlightStyle, { fallback: true }),
  presetLanguage(),
  EditorView.lineWrapping,
  EditorView.theme({
    '&': { height: '100%', fontSize: '12px' },
    '.cm-scroller': { fontFamily: 'var(--mono)' },
  }),
]

export function PresetEditor({ path, text, problems, onSave }: PresetEditorProps): JSX.Element {
  const host = useRef<HTMLDivElement>(null)
  const view = useRef<EditorView>()
  const [dirty, setDirty] = useState(false)
  /** The text the player last loaded, so a save can be compared against it. */
  const loaded = useRef<string | undefined>(text)

  // `onSave` is read through a ref so a new callback identity does not tear the
  // view down and rebuild it, which would drop the undo history.
  const onSaveRef = useRef(onSave)
  useEffect(() => {
    onSaveRef.current = onSave
  }, [onSave])

  useEffect(() => {
    if (host.current === null) return
    const editor = new EditorView({
      parent: host.current,
      state: EditorState.create({
        // Empty on purpose: the document is pushed in by the effect below, on
        // mount and on every reload alike, so there is one path that puts text
        // into this view rather than two that have to agree.
        doc: '',
        extensions: [
          ...BASE,
          keymap.of([
            {
              key: 'Mod-s',
              preventDefault: true,
              run: (target) => {
                onSaveRef.current(target.state.doc.toString())
                setDirty(false)
                return true
              },
            },
            ...defaultKeymap,
            ...historyKeymap,
          ]),
          EditorView.updateListener.of((update) => {
            if (update.docChanged) setDirty(update.state.doc.toString() !== loaded.current)
          }),
        ],
      }),
    })
    view.current = editor
    // CodeMirror is not a React resource: without this the view, its DOM and
    // its keymap outlive the component (ADR-0178).
    return () => {
      editor.destroy()
      view.current = undefined
    }
    // Built once. The document and the markers are pushed in below.
  }, [])

  // A different file, or the same one reloaded. The author's unsaved edit wins
  // over a reload of the same file; a genuinely different file always replaces.
  const shown = useRef<string | undefined>(path)
  useEffect(() => {
    const editor = view.current
    if (editor === undefined || text === undefined) return
    const switched = shown.current !== path
    if (!switched && dirty) return
    shown.current = path
    loaded.current = text
    editor.dispatch({
      changes: { from: 0, to: editor.state.doc.length, insert: text },
    })
    setDirty(false)
  }, [path, text, dirty])

  useEffect(() => {
    const editor = view.current
    if (editor === undefined) return
    const doc = editor.state.doc.toString()
    const diagnostics: Diagnostic[] =
      path === undefined
        ? []
        : markersFor(doc, path, problems).map((marker) => ({
            from: marker.from,
            to: marker.to,
            severity: marker.severity,
            message: marker.message,
          }))
    editor.dispatch(setDiagnostics(editor.state, diagnostics))
  }, [path, problems, text])

  return (
    <div className={styles.editor}>
      <div className={styles.bar}>
        <span className={styles.path}>{path ?? 'no file'}</span>
        {dirty && <span className={styles.dirty}>unsaved — Ctrl+S writes it</span>}
      </div>
      <div className={styles.host} ref={host} aria-label="preset file" />
    </div>
  )
}
