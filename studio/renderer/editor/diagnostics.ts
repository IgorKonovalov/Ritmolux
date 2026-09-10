/**
 * The player's complaints, placed on the lines of the file the author is
 * editing.
 *
 * **The player is the only authority on whether a preset is wrong.** The studio
 * runs no parser and no expression compiler of its own — a second one would
 * disagree with the engine on exactly the inputs that matter, and the author
 * would be told a thing the picture contradicts. So a marker exists only where
 * an event put it.
 *
 * Two shapes arrive. A TOML syntax failure carries a line and column, and is
 * marked there. An expression failure carries neither, because it is raised
 * after the document was parsed into values that no longer hold a position, and
 * carries the **parameter's name** instead (spec 0003) — which is enough to
 * find its line in the `[params]` table.
 *
 * Kept free of any CodeMirror import so it can be tested as what it is: a
 * function from a file's text and a list of events to a list of ranges.
 */
import { readParams } from '@shared/toml'

export interface PlayerProblem {
  file: string
  message: string
  line: number | null
  col: number | null
  param: string | null
  kind: 'error' | 'warning'
}

export interface Marker {
  from: number
  to: number
  severity: 'error' | 'warning'
  message: string
}

/** The `[start, end)` offsets of one-based `line` in `text`. */
function lineSpan(text: string, line: number): { from: number; to: number } | undefined {
  const lines = text.split('\n')
  if (line < 1 || line > lines.length) return undefined
  let from = 0
  for (let i = 0; i < line - 1; i += 1) from += lines[i].length + 1
  // Excluding the carriage return, so a marker on a CRLF file does not
  // underline an invisible character past the end of the text.
  return { from, to: from + lines[line - 1].replace(/\r$/, '').length }
}

/**
 * Markers for `text`, from the problems the player reported about `path`.
 *
 * Problems about another file are dropped rather than placed at the top of this
 * one: a preset directory reloads as a whole, so an error in a sibling is
 * routine and marking it here would blame the wrong document.
 *
 * A problem this file cannot be shown to have — a line past its end, a
 * parameter it does not bind — is dropped too. That case is real and not
 * defensive: the events describe the file **as the player last read it**, and
 * the author has been typing since.
 */
export function markersFor(text: string, path: string, problems: PlayerProblem[]): Marker[] {
  const mine = problems.filter((problem) => samePath(problem.file, path))
  const bindings = readParams(text)
  const out: Marker[] = []

  for (const problem of mine) {
    const span =
      problem.line !== null
        ? lineSpan(text, problem.line)
        : problem.param === null
          ? undefined
          : spanOfBinding(text, bindings, problem.param)
    if (span === undefined) continue
    out.push({
      from: span.from,
      to: span.to,
      severity: problem.kind,
      message: problem.message,
    })
  }
  return out
}

function spanOfBinding(
  text: string,
  bindings: ReturnType<typeof readParams>,
  param: string,
): { from: number; to: number } | undefined {
  const binding = bindings.find((candidate) => candidate.name === param)
  return binding === undefined ? undefined : lineSpan(text, binding.line + 1)
}

/**
 * Whether two paths name one file.
 *
 * Compared case-insensitively with both separators normalised: the player
 * prints a path the operating system gave it and the studio holds the one it
 * opened, and on Windows those routinely differ in case and in slash without
 * being different files. Getting this wrong shows an author no marker at all
 * and gives them nothing to notice.
 */
function samePath(a: string, b: string): boolean {
  const normal = (path: string): string => path.replace(/\\/g, '/').toLowerCase()
  return normal(a) === normal(b)
}
