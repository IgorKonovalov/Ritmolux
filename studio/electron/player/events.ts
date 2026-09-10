/**
 * Standard error to `PlayerEvent`s.
 *
 * The player interleaves two things on one stream: its human diagnostics, and —
 * with `--events` — one JSON object per line. The spec's framing rule is that
 * **no human line may begin with `{`**, so the first byte routes a line with no
 * framing of its own. Everything that does begin with `{` is parsed and then
 * validated against the protocol schema; a line that fails either step is
 * counted and reported, never thrown. A malformed line between two good ones
 * must not cost the good ones — the player is mid-show and the studio's job is
 * to keep reading.
 */
import { playerEventSchema, type PlayerEvent } from '@shared/protocol'

export interface EventReaderSinks {
  /** A line that parsed and validated. */
  onEvent: (event: PlayerEvent) => void
  /** A line that did not begin with `{`: the player's own diagnostics. */
  onDiagnostic: (line: string) => void
  /** A line that began with `{` and was not a valid event. */
  onMalformed: (line: string, reason: string) => void
}

/**
 * Splits a chunk stream into lines and routes each one.
 *
 * Holds the partial trailing line between chunks: a pipe splits wherever it
 * likes, and an event line arriving in two reads is ordinary, not an error.
 */
export class EventReader {
  private partial = ''
  private malformedCount = 0

  constructor(private readonly sinks: EventReaderSinks) {}

  get malformed(): number {
    return this.malformedCount
  }

  push(chunk: string): void {
    this.partial += chunk
    let newline = this.partial.indexOf('\n')
    while (newline !== -1) {
      this.line(this.partial.slice(0, newline))
      this.partial = this.partial.slice(newline + 1)
      newline = this.partial.indexOf('\n')
    }
  }

  /** The child exited; anything held back without a newline is still a line. */
  flush(): void {
    if (this.partial.length > 0) {
      this.line(this.partial)
      this.partial = ''
    }
  }

  private line(raw: string): void {
    const line = raw.endsWith('\r') ? raw.slice(0, -1) : raw
    if (line.length === 0) return
    if (!line.startsWith('{')) {
      this.sinks.onDiagnostic(line)
      return
    }
    let parsed: unknown
    try {
      parsed = JSON.parse(line)
    } catch (error) {
      this.malformedCount += 1
      this.sinks.onMalformed(line, error instanceof Error ? error.message : 'unparseable JSON')
      return
    }
    const result = playerEventSchema.safeParse(parsed)
    if (!result.success) {
      this.malformedCount += 1
      this.sinks.onMalformed(line, result.error.issues.map((i) => i.message).join('; '))
      return
    }
    this.sinks.onEvent(result.data)
  }
}
