/**
 * The `[palette]` table: a built-in by name, or the preset's own stops.
 *
 * The built-in names come from the schema's `palette.name` enum, so the list is
 * the engine's and not a copy of it (ADR-0170). The stops come from the file,
 * and each edit rewrites the line that stop is already on — the shipped
 * palettes carry a comment per stop naming the colour, and those are the
 * author's record.
 *
 * There is **no preview of the ramp on a figure here**. The studio draws no
 * pixels: what a palette looks like on a preset is the player's answer, and it
 * arrives in the preview a reload later (ADR-0175). The gradient below is the
 * ramp itself, which is a fact about the stops rather than a rendering of the
 * scene.
 */
import type { SchemaDocument } from '@shared/schema'
import type { Palette } from '@shared/toml'

import { StopHandle } from './StopHandle'

import styles from './PaletteEditor.module.css'

export interface PaletteEditorProps {
  document: SchemaDocument
  palette: Palette
  writable: boolean
  onName: (name: string) => void
  onMove: (index: number, at: number) => void
  onCommit: (index: number, at: number) => void
  onRecolour: (index: number, color: string) => void
}

/** The built-in palette names, as the engine's own `palette.name` enum. */
export function builtInPalettes(document: SchemaDocument): string[] {
  const table = document.tables.find((candidate) => candidate.name === 'palette')
  return table?.keys.find((key) => key.name === 'name')?.values ?? []
}

/** The ramp the stops describe, as a CSS gradient. */
function ramp(palette: Palette): string {
  if (palette.stops.length === 0) return 'var(--panel)'
  const sorted = [...palette.stops].sort((a, b) => a.at - b.at)
  const parts = sorted.map((stop) => `${stop.color} ${(stop.at * 100).toFixed(1)}%`)
  return `linear-gradient(to right, ${parts.join(', ')})`
}

export function PaletteEditor({
  document,
  palette,
  writable,
  onName,
  onMove,
  onCommit,
  onRecolour,
}: PaletteEditorProps): JSX.Element {
  const names = builtInPalettes(document)
  const custom = palette.stops.length > 0

  return (
    <section className={styles.palette}>
      <h2 className={styles.heading}>palette</h2>

      <div className={styles.row}>
        <label className={styles.label} htmlFor="palette-name">
          built-in
        </label>
        <select
          id="palette-name"
          className={styles.select}
          value={palette.name ?? ''}
          disabled={!writable || custom}
          onChange={(event) => onName(event.currentTarget.value)}
        >
          <option value="" disabled>
            {custom ? 'this preset defines its own stops' : 'none'}
          </option>
          {names.map((name) => (
            <option key={name} value={name}>
              {name}
            </option>
          ))}
        </select>
      </div>

      {custom && (
        <>
          <div className={styles.ramp} style={{ background: ramp(palette) }} aria-hidden="true" />
          {palette.stops.map((stop, index) => (
            <StopHandle
              key={stop.line}
              index={index}
              at={stop.at}
              color={stop.color}
              onMove={onMove}
              onCommit={writable ? onCommit : () => undefined}
              onRecolour={writable ? onRecolour : () => undefined}
            />
          ))}
        </>
      )}

      {!custom && names.length === 0 && (
        <p className={styles.note}>
          The schema this player printed declares no palette names, so there is nothing to choose
          from.
        </p>
      )}
    </section>
  )
}
