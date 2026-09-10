/**
 * A new preset, built from what the engine declares rather than from a stored
 * example.
 *
 * **Generated, never stored.** A checked-in template per system would be a
 * second copy of the engine's defaults, and it would go stale silently the
 * first time a default moved — which is the drift ADR-0170 exists to end. So a
 * template is written from the schema document the player just printed: its
 * system key, and the structural tables that system requires.
 *
 * A template deliberately binds **no parameters**. Every parameter already has
 * the default the engine declares, and writing them out would produce a
 * hundred-line file whose every line says what would have happened anyway; an
 * author would then have to work out which lines they had actually chosen. The
 * required structural tables are different — the preset does not load without
 * them.
 */
import type { SchemaDocument, TableSpec } from './schema'

/**
 * What a system needs beyond its key before it will load.
 *
 * The two entries are a **fixture of the engine's own refusals**, not a
 * catalogue of the systems: the L-system's grammar has no default axiom or rule
 * set to fall back on, and the star pattern's has no default tiling or rings.
 * Every other system loads from its key alone. The Rust suite asserts that
 * these are the only two, so a third would fail there rather than produce a
 * template here that does not load.
 */
const REQUIRED: Record<string, string> = {
  lsystem: [
    '[generator]',
    'axiom     = "F"',
    'rules     = { F = "F[+F]F[-F]F" }',
    'angle_deg = 22',
    'max_depth = 4',
  ].join('\n'),
  star_pattern: [
    '[generator]',
    'tiling = "none"',
    'rings  = [',
    '  { motif = "circle",  count = 1, radius = 0.00, scale = 0.22 },',
    '  { motif = "scallop", count = 7, radius = 0.34, scale = 0.20 },',
    ']',
  ].join('\n'),
}

export interface TemplateOptions {
  /** The preset's `name`; the file's own name is the caller's business. */
  name: string
  system: string
}

/** Whether the schema declares this system, so a template can be built for it. */
export function canTemplate(document: SchemaDocument, system: string): boolean {
  return document.systems.some((roster) => roster.name === system)
}

/**
 * A preset document for `system`, ready to write into the watched directory.
 *
 * Throws for a system the schema does not declare: writing a file the player
 * will refuse, and then showing the author the refusal, is worse than refusing
 * to write it.
 */
export function templateFor(document: SchemaDocument, options: TemplateOptions): string {
  if (!canTemplate(document, options.system)) {
    throw new Error(`\`${options.system}\` is not a system this player declares`)
  }
  const header = [
    `# ${options.name}`,
    '#',
    '# Built from the engine defaults. Every parameter this system declares is',
    '# already at its default; bind one under [params] to make it yours.',
    '',
    `name   = ${JSON.stringify(options.name)}`,
    `system = ${JSON.stringify(options.system)}`,
    '',
    '[params]',
  ].join('\n')

  const required = REQUIRED[options.system]
  return required === undefined ? `${header}\n` : `${header}\n\n${required}\n`
}

/**
 * A file name for a preset called `name`, inside the watched directory.
 *
 * Lowercased, with everything that is not a letter, a digit or an underscore
 * folded to one underscore: the name is the author's and the file name has to
 * survive two filesystems. An empty result is refused rather than turned into
 * a file called `.toml`.
 */
export function fileNameFor(name: string): string {
  const stem = name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '')
  if (stem === '') throw new Error('a preset needs a name with a letter or a digit in it')
  return `${stem}.toml`
}

/** The structural tables, which are every table but the document's own two. */
export function structuralTables(document: SchemaDocument): TableSpec[] {
  // `preset` is the document itself and `stop` is an element of `palette`'s
  // list rather than a table an author writes; both are described by the
  // editors around them rather than edited as tables of their own.
  return document.tables.filter((table) => table.name !== 'preset' && table.name !== 'stop')
}

/**
 * `fileName` inside `dir`, joined with the separator that directory already
 * uses.
 *
 * Written here rather than with `node:path` because the renderer is sandboxed
 * and has no Node at all (ADR-0178). Both paths come from the player, so the
 * separator is the operating system's own and reading it off the string is
 * exact rather than a guess about the platform.
 */
export function presetPath(dir: string, fileName: string): string {
  const separator = dir.includes('\\') && !dir.includes('/') ? '\\' : '/'
  return `${dir.replace(/[\\/]+$/, '')}${separator}${fileName}`
}
