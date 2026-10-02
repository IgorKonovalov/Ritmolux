/**
 * `ffmpeg -progress`'s stream, read into a frame count (ADR-0262).
 *
 * The encoder writes blocks of `key=value` lines, each closed by a
 * `progress=continue` line and the last by `progress=end`. The text arrives in
 * whatever chunks the pipe hands over, so a line is only read once its newline
 * has arrived — a `frame=12` cut after the `1` would otherwise read as frame 1.
 *
 * The count is **monotone**: a value lower than one already seen is ignored, so
 * the bar never walks backwards whatever the encoder printed. `done` turns true
 * on `progress=end` and on nothing else; an encoder that stopped writing without
 * one has not finished, whatever its count reached.
 */
export class ProgressParser {
  private pending = ''
  private latest = 0
  private ended = false

  /** Frames the encoder has written so far. */
  get frame(): number {
    return this.latest
  }

  /** Whether the encoder reported the end of its stream. */
  get done(): boolean {
    return this.ended
  }

  /** Feed one chunk; returns whether the frame count moved. */
  push(chunk: string): boolean {
    this.pending += chunk
    const lines = this.pending.split('\n')
    this.pending = lines.pop() ?? ''
    const before = this.latest
    for (const raw of lines) {
      const line = raw.trim()
      const eq = line.indexOf('=')
      if (eq <= 0) continue
      const key = line.slice(0, eq)
      const value = line.slice(eq + 1).trim()
      if (key === 'frame') {
        const frame = Number(value)
        if (Number.isInteger(frame) && frame > this.latest) this.latest = frame
      } else if (key === 'progress' && value === 'end') {
        this.ended = true
      }
    }
    return this.latest !== before
  }
}

/**
 * The last `max` lines of a child's standard error, for a failure message and
 * the render log. Bounded so a chatty child cannot grow the main process.
 */
export class LineTail {
  private pending = ''
  private readonly kept: string[] = []

  constructor(
    private readonly max = 20,
    private readonly onLine?: (line: string) => void,
  ) {}

  push(chunk: string): void {
    this.pending += chunk
    const lines = this.pending.split(/\r?\n|\r/)
    this.pending = lines.pop() ?? ''
    for (const line of lines) this.keep(line)
  }

  /** The lines kept, the unterminated last one included. */
  lines(): string[] {
    return this.pending === '' ? [...this.kept] : [...this.kept, this.pending].slice(-this.max)
  }

  private keep(line: string): void {
    if (line.trim() === '') return
    this.onLine?.(line)
    this.kept.push(line)
    if (this.kept.length > this.max) this.kept.shift()
  }
}
