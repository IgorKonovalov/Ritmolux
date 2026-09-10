/**
 * The file behind the preset on screen: its text, its bindings, and the write.
 *
 * The path is the player's own (`preset.file`), never one the studio resolved —
 * a second resolver is what ADR-0184 refuses, and the failure it prevents is a
 * studio editing files the player is not watching while both report success.
 *
 * Re-read on every reload. The `roster` event fires on each one, so after a
 * write the text that comes back is what the player actually loaded rather than
 * what the studio believes it wrote; an edit made in another editor lands the
 * same way.
 *
 * **A preset the studio did not create is never written to** (ADR-0189). The
 * first gesture against one is held here: the document that gesture produced is
 * captured, the surface asks for a name, and the whole document lands in a new
 * file. What this session created it writes silently from then on, at the
 * cadence a live editor needs — one write per gesture, atomically.
 *
 * **The session is the unit.** The set of forks below lives as long as the
 * window, so a relaunch prompts once more for a preset this studio wrote
 * yesterday. The studio has nowhere durable to keep that set, and a marker
 * inside the author's document would be studio state in their file; the price
 * is one extra file, and no edit is lost paying it.
 */
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'

import { fileNameFor, presetPath } from '@shared/templates'
import {
  readPalette,
  readParams,
  setConstant,
  setKey,
  type Binding,
  type Palette,
} from '@shared/toml'

export type PresetFileState =
  /** No preset reported yet. */
  | { status: 'waiting' }
  /**
   * On screen, but from the embedded set, which has no file (ADR-0184).
   *
   * Editable all the same: the fork is its first file. What that fork is built
   * from is the caller's `base`, because no event carries the embedded
   * document's own text and the studio resolves nothing of its own.
   */
  | { status: 'embedded' }
  | { status: 'loading'; path: string }
  | { status: 'ready'; path: string; text: string }
  | { status: 'failed'; path: string; reason: string }

/** A gesture held for a name, and what the surface says about it. */
export interface ForkRequest {
  /** The preset the gesture was made against. */
  source: string
  /** What the name field starts at. */
  suggestion: string
}

/** A held gesture, with the document it produced. */
interface HeldGesture extends ForkRequest {
  /**
   * The whole document, captured when the gesture fired and never re-read
   * afterwards — a preset change between the gesture and the answer must not
   * redirect the write (ADR-0189).
   */
  text: string
}

export interface ActivePresetOptions {
  /** Where the watcher is looking, and so where a fork can land. */
  dir: string | null
  /** The preset's name as the player reports it, for the suggestion. */
  name: string | undefined
  /** The document an embedded preset forks from, when there is one to build. */
  base: string | undefined
}

export interface ActivePreset {
  file: PresetFileState
  /**
   * The document a gesture edits: the file's text, the base behind an embedded
   * preset, or — while a fork is held — what the held gestures have produced so
   * far, so a second edit made before the name is typed accumulates rather than
   * replacing the first.
   */
  text: string | undefined
  /** The `[params]` bindings of that document. */
  bindings: Binding[]
  /** Its `[palette]`. */
  palette: Palette
  /** Whether there is both a document to edit and somewhere to put it. */
  writable: boolean
  /**
   * Write `name = value` into the document, and answer with the reason it did
   * not happen.
   *
   * The override is **not** cleared here: the player drops every override on a
   * reload (spec 0003), and clearing before the reload lands would snap the
   * picture back to the old value for as long as the watcher's poll takes.
   */
  commit: (name: string, value: number) => Promise<string | undefined>
  /**
   * Write a whole document back.
   *
   * What the text editor's save and every `[palette]` edit go through: those
   * rewrite a line this hook does not model, so the caller builds the next
   * document with the editors in `@shared/toml` and hands it over whole.
   */
  write: (next: string) => Promise<string | undefined>
  /** The gesture waiting for a name, if there is one. */
  fork: ForkRequest | undefined
  /** Answer it: the captured document lands in a new file called `name`. */
  keep: (name: string) => Promise<string | undefined>
  /** Drop it: nothing is written, and the held edits go with it. */
  discard: () => void
  /**
   * Write a document to a file that does not exist yet, and remember it as this
   * session's own — what the library's new preset goes through, so editing what
   * was just created does not ask again.
   */
  create: (fileName: string, text: string) => Promise<string | undefined>
}

export function useActivePreset(
  file: string | null | undefined,
  reloads: number,
  { dir, name, base }: ActivePresetOptions,
): ActivePreset {
  const [state, setState] = useState<PresetFileState>({ status: 'waiting' })
  const [held, setHeld] = useState<HeldGesture>()
  /**
   * The files this session wrote, which are the only ones it writes again
   * without asking.
   *
   * Compared as the player spells them, with no case folding: both sides come
   * from the same `roster.dir`, and folding would let `Ink.toml` pass for a
   * fork called `ink.toml` on a filesystem where those are two presets — which
   * is the silent overwrite this whole path exists to remove. A fork that goes
   * unrecognised costs one extra prompt.
   */
  const ours = useRef(new Set<string>())

  useEffect(() => {
    if (file === undefined) {
      setState({ status: 'waiting' })
      return
    }
    if (file === null) {
      setState({ status: 'embedded' })
      return
    }
    let live = true
    setState({ status: 'loading', path: file })
    void window.api.preset.read(file).then((result) => {
      if (!live) return
      setState(
        result.ok
          ? { status: 'ready', path: result.value.path, text: result.value.text }
          : { status: 'failed', path: file, reason: result.reason },
      )
    })
    return () => {
      live = false
    }
  }, [file, reloads])

  const loaded =
    state.status === 'ready' ? state.text : state.status === 'embedded' ? base : undefined
  const text = held?.text ?? loaded
  const bindings = useMemo(() => (text === undefined ? [] : readParams(text)), [text])
  const palette = useMemo(
    () => (text === undefined ? { name: undefined, stops: [] } : readPalette(text)),
    [text],
  )

  const create = useCallback(
    async (fileName: string, document: string): Promise<string | undefined> => {
      if (dir === null) return 'this run resolved no preset directory, so there is nowhere to save'
      const target = presetPath(dir, fileName)
      const result = await window.api.preset.create(target, document)
      if (!result.ok) return result.reason
      ours.current.add(target)
      return undefined
    },
    [dir],
  )

  /**
   * Where a gesture's document goes: the file itself when this session made it,
   * or a held fork when it did not.
   */
  const land = useCallback(
    async (next: string): Promise<string | undefined> => {
      const path = state.status === 'ready' ? state.path : undefined
      if (path !== undefined && ours.current.has(path)) {
        const result = await window.api.preset.write(path, next)
        return result.ok ? undefined : result.reason
      }
      setHeld({
        text: next,
        source: name ?? path ?? 'this preset',
        suggestion: `${name ?? 'preset'} copy`,
      })
      return undefined
    },
    [state, name],
  )

  const commit = useCallback(
    async (parameter: string, value: number): Promise<string | undefined> => {
      if (text === undefined) return 'there is no preset to edit'
      let next: string
      try {
        next = setConstant(text, parameter, value)
      } catch (error) {
        return (error as Error).message
      }
      // Nothing to write when the value is already what the document says: a
      // drag that ends where it started must not touch an mtime and make the
      // player reload for nothing — nor ask for a name.
      if (next === text) return undefined
      return land(next)
    },
    [text, land],
  )

  const write = useCallback(
    async (next: string): Promise<string | undefined> => {
      if (text === undefined) return 'there is no preset to edit'
      if (next === text) return undefined
      return land(next)
    },
    [text, land],
  )

  const keep = useCallback(
    async (chosen: string): Promise<string | undefined> => {
      if (held === undefined) return undefined
      let document: string
      let fileName: string
      try {
        // The fork carries its own `name`, so the roster does not hold two
        // presets under one name and `ctl/preset` stays unambiguous.
        document = setKey(held.text, '', 'name', JSON.stringify(chosen))
        fileName = fileNameFor(chosen)
      } catch (error) {
        return (error as Error).message
      }
      const reason = await create(fileName, document)
      // The request stays up on a refusal — a name already taken is answered by
      // typing another one, not by losing the gesture.
      if (reason === undefined) setHeld(undefined)
      return reason
    },
    [held, create],
  )

  const discard = useCallback(() => setHeld(undefined), [])

  return {
    file: state,
    text,
    bindings,
    palette,
    writable: text !== undefined && dir !== null,
    commit,
    write,
    fork: held,
    keep,
    discard,
    create,
  }
}
