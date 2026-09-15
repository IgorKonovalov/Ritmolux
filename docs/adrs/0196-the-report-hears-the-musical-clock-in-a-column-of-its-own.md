# ADR-0196 — The report hears the musical clock in a column of its own, and every existing column keeps its stimulus

> **Status:** accepted 2026-09-15 (Plan 0182), with an Outcome
> **Date:** 2026-09-14
> **Related plan(s):** [0182](../plans/done/0182-the-report-hears-a-counter.md)
> **Supplements:** [0134](0134-motion-is-two-readings-and-anchoring-is-why-neither-can-be-a-threshold.md)
> (the `drive` column, whose stimulus this leaves alone), [0042](0042-reachability-measured-on-the-expression-tree.md)
> (the report's stimuli are a recorded judgement, and their history is kept comparable),
> [0050](0050-downbeat-and-phrase-tracking-with-confidence-fallback.md) and
> [0109](0109-the-beat-clock-counts-onsets-not-beats.md) (what the clock variables are)

## Context

`shot --report` measures every column except `rate` and the transient pair by capturing a **held**
`AnalysisFrame`: one frame, repeated for 24 or 48 frames, differenced against a silent capture of
the same depth (`standalone/src/shot/report.rs`, `build_family_report`). A held frame holds its
counters. `band_stimuli_at` and `AnalysisFrame::fully_driven` leave `beat_index`, `bar_index`,
`beat_in_bar`, `bar_phase` and `time_since_beat` at their `Default` zeros, so across a capture the
musical clock never moves.

That makes a whole class of binding invisible. Backlog 0192 found it on `shape_maple` and
`shape_lion`, which step their ring colours one band per onset with
`color_center = mod(0.875 + beat_index / 16, 1)`. The report prints `onset 0.000` and `mid 0.000`
for them, although they are among the most rhythmically driven presets in the set. The class is not
small. On 2026-09-14, **27** shipped presets bind at least one counter outside a comment:
`beat_index` in 19, `bar_index` in 6, `time_since_beat` in 3 and `bar_phase` in 2. And eight carry a
`[hold]` table, whose `bar` edge (ADR-0180 rule 2) re-samples only when `bar_index` changes, which
it never does in a held capture.

The obvious repair has a cost. The report's numbers are what the close ceremony's curation step
reads and what commit messages, ADR Outcomes and backlog entries quote. ADR-0042 kept the full-scale
stimuli unchanged when it added realistic levels for exactly that reason: a stimulus that moves
under a column makes every earlier reading of that column incomparable with every later one.

The table also has no room. Its widest row, a line family with the `geom` column present, is 99
characters, and `no_report_table_line_wraps_at_a_hundred_columns` holds it under 100. A new
six-wide cell with its gutter adds 7.

## Decision

We will add a **`count`** column to `--report`, placed immediately after `onset`. It is measured
from a stimulus of its own and leaves every existing column's stimulus, capture and number exactly
as they are.

1. **The stimulus is a synthetic musical clock at silence.** It is a sequence of
   `REPORT_FRAMES_LATE` (48) frames. Every level, the band array and `bpm` stay at
   `AnalysisFrame::default()`. Only the clock moves: a beat lands every **5** frames, from frame 5.
   On a beat frame `beat` is `true`, `beat_index` steps and `time_since_beat` is 0.
   `bar` ramps across each beat, `beat_in_bar` is `beat_index mod 4`, `bar_index` is
   `beat_index / 4`, and `bar_phase` ramps across the bar. Over 48 frames that is 9 beats and two
   bar edges, at frames 20 and 40, so `bar_index` takes three values.
2. **The event and the counter move together.** A frame where `beat_index` steps without `beat`
   firing is not a frame the analyzer produces. The report already refuses that kind of fixture:
   `band_stimulus` lights the spectrum slice beside every band scalar for the same reason. So a
   preset that reads only the `beat` flag also shows in `count`, as well as in `onset`. That is
   true: it does respond to the clock.
3. **The reading is frame-aligned against silence.** `count` is the mean, over the 48 frames, of
   `frame_diff` between frame *i* of the clock capture and frame *i* of a silent capture. Both are
   taken through `capture_preset_over`, from the same reset and at the same `FALLBACK_DT`. So the
   two sequences differ in nothing but the clock fields. It is a mean rather than one settled
   depth, because a periodic binding such as `mod(bar_index, 2)` agrees with silence at some
   depths. A single sample at one of those depths would read zero.
4. **The cadence is not a tempo, on purpose.** Five frames at 1/60 s is 720 BPM. The column asks
   whether the clock reaches the picture at all, the way `FULL_LEVELS` asks about peaks rather than
   typical levels. A musical tempo would need about 30 frames per beat and four beats per bar edge,
   so a 240-frame sequence captured twice, with a readback on every frame. That is several times
   the rendering of every other column added together.
5. **`geom` leaves the main table** for a one-column block under the prose that already explains
   it. Its JSON key does not change. Without `geom` the widest row is 92 characters, and with
   `count` it is 99, inside the existing test's bound. `count` sits in the main table because the
   misreading in backlog 0192 happened there: a curator scanning `onset 0.000` needs the
   neighbouring cell, not a block further down.
6. **`--json` carries it as its own key**, `"count": { "mean", "frames", "frames_per_beat" }`,
   placed after `"drive"`. The schedule travels with the number, as `rate` carries
   `measured_at_px`. The four-key `reactivity` object keeps its key set.

## Consequences

### Positive

- **A counter-driven preset stops reading as inert.** `shape_maple` and `shape_lion` get a non-zero
  cell beside their zero `onset`, and a bar-held binding becomes visible for the first time.
- **No historical number moves.** Every earlier column keeps its stimulus and its capture. A
  before-and-after `--json` on the shipped library differs by the new key and nothing else, so
  every curation verdict already recorded keeps meaning what it said.
- **A clock-free preset reads exactly zero, by construction rather than below a noise floor.** Two
  captures of the same inputs on one machine and binary are byte-identical, which
  `docs/capturing.md` already states. A preset whose expressions read no clock variable and hold on
  no musical edge therefore produces two identical sequences. Its `count` is `0.0`, not merely a
  small number.

### Negative

- **The report gets slower, and by more than the frame count suggests.** Each preset adds two
  48-frame sequences, 96 frames against the 312 its other captures render at 192 px. That is about
  a third more rendering. But `capture_preset_over` reads back every frame, so those 96 frames add
  96 readbacks where the rest of a preset's captures at that size take 11. The wall-time increase
  is unmeasured until Plan 0182 Phase 1 takes it, and readback could dominate.
- **720 BPM misrepresents anything the clock drives continuously.** A `time_since_beat` envelope
  sees only the first twelfth of a second of its decay, and a `[smoothing]` constant longer than a
  beat averages a stepped binding into a glide. The column still reads such a preset as responsive,
  and it is right to. But its magnitude is not what the preset does at a real tempo. Like `drive`,
  it is a number read against family neighbours and never a threshold.
- **`count` is a different statistic from the four band columns beside it.** It is a mean over a
  sequence, not one settled depth, so a `count` cell and an `onset` cell of the same size do not
  mean the same amount of response. The docs have to say so where the columns are explained.
- **Line families lose `geom` from the main table.** An author tuning `scale` reads it one block
  lower than before.
- **`bpm` stays at 0.** A binding gated on `tempo` is as invisible to this column as it is to every
  other one. The reachability walk's real-analyzer frames remain the only place a tempo gate is
  exercised.

### Neutral

- Backlog 0192's probe (`absent: beat_index:` in `report.rs`) goes red on delivery, because the
  stimulus sets that field. That is the entry closing, not decaying.

## Outcome (2026-09-15, at Plan 0182's close)

**The Decision landed as written.**

- **Column and JSON.** `standalone/src/shot/report.rs` builds the stimulus in `clock_stimulus`. It
  reads `count` as `mean_aligned_diff` over two `capture_preset_over` sequences, taken after every
  other column's captures. It prints the cell after `onset` and moves `geom` to a one-column block.
  `--json` writes `"count":{"mean","frames":48,"frames_per_beat":5}` after `"drive"`, with the mean
  at full precision so an exact zero reads `0`.
- **Readings.** A clock-free fixture reads exactly `0`. A `beat_index` hue and a `bar`-held hue both
  read above zero. `Path Maple` and `Path Lion` read `count 0.169` and `0.191` beside an unchanged
  `onset 0.000`.
- **No historical number moved.** Plan 0182 Phase 1's log records that a before-and-after
  `--report --json` over `presets/` differs by the `count` key alone.
- **The first Negative is now measured.** Wall time of `shot --presets presets --report` went from
  163.4 s to 203.6 s on the reference machine, +25 %. Readback did not dominate.

**The Neutral consequence did not happen.** Backlog 0192's probe
`absent: beat_index: in: standalone/src/shot/report.rs` stayed green on the fixed tree. The
stimulus sets the field in struct shorthand (`beat_index,`), and the probe matched only the
`beat_index:` form. The entry was archived at the close as discharged, and the probe retired with
it.

**Open:** `report.rs` mirrors core's crate-private `FALLBACK_DT` as `CAPTURE_DT` to give
`time_since_beat` its seconds. Nothing holds the two equal.

## Alternatives considered

### Alternative A — Advance the counters inside the `drive` capture only

No new column: the fully driven capture would also run the clock. **Rejected because it moves the
`drive` number for every preset that reads a counter.** Every `drive` value quoted since ADR-0134 is
then incomparable with every later one, which is the exact cost ADR-0042 kept the full-scale stimuli
unchanged to avoid. It would also mix two questions into one cell, "responds to everything at once"
and "responds to the clock", and give a reader no way to separate them.

### Alternative B — Leave the instrument, document the blind spot

Add a row to `docs/testing.md` and a line to the preset-author skill. **Rejected because the
misreading has already happened** to the author best placed to avoid it. A documented blind spot is
read once. A zero in a table is read every session.

### Alternative C — Ramp the counters without firing `beat`

Keep the column purely about counters by leaving the event flag false. **Rejected because the frame
is not one the analyzer can produce.** `beat_index` counts onset detections and steps exactly on
the hop where `beat` fires (ADR-0109). A preset written against the real coupling, for example one
that latches on `beat` and reads `beat_index` for a colour, would be measured on a fixture that
separates the two.

### Alternative D — A musical tempo

About 30 frames per beat, and 240 frames to cross two bar edges. **Rejected on cost** (Decision item
4). Shortening the sequence at that tempo gives the other failure: `bar_index` never steps, so
bar-driven bindings and bar holds stay invisible, which is the half of the class this column exists
to reach.

### Alternative E — Sample the last frame only

One depth, like every other column. **Rejected because a periodic binding aliases to silence** at
some depths. `mod(bar_index, 2)` at frame 47, where `bar_index` is 2, reads exactly the silent
value.
