# 0037 — Verifying easing: a transient probe, a signal with dynamics, and the levels authors calibrate against

> **Status:** approved
> **Created:** 2026-07-26
> **Approved:** 2026-07-26 — ready for `dev` (a fresh session; the handoff is manual on purpose)
> **Owner skill(s):** dev, human
> **Related ADRs:** [0039](../adrs/0039-verify-easing-with-a-transient-probe-not-a-committed-clip.md)
> (this plan's decision), [0035](../adrs/0035-asymmetric-attack-release-easing.md) (the capability it
> makes checkable), [0019](../adrs/0019-eased-parameters.md) (the `[smoothing]` surface)
> **Backlog entries closed:** [0013](../design-backlog.md), the unresolved half of
> [0008](../design-backlog.md), plus [0012](../design-backlog.md) and [0014](../design-backlog.md) as
> documentation

## TL;DR

`[smoothing]` cannot be verified by anything except a human watching the app, because the capture
primitive holds one stimulus for every frame and every synthesized signal is a steady tone. This
plan gives the capture path a **time-varying stimulus**, adds a **step-response probe** whose
rise-versus-fall ratio makes ADR-0035's asymmetric easing directly observable, and adds one
**synthesized generator with musical dynamics**. The calibration question — what levels real material
actually produces — is closed by a `human` phase that measures the user's own audio and records the
numbers, rather than by committing a clip.

## Context & problem

ADR-0035 shipped `{ attack, release }`; 20 presets adopted it in `a070f5a` and more easing changed in
`8b5b2e0` and `66300d6`. **Not one of those edits was checked by anything automated.** Two mechanisms
cause that, and they are independent — fixing either alone leaves the gap open.

**1. The capture primitive holds one stimulus.** `Renderer::capture_preset(name, frame, frames)`
(`core/src/render/mod.rs:1397`) takes a single `&AnalysisFrame` and renders `frames` of it, so every
smoother converges before the pixels are read. `--report` is built entirely on that call
(`standalone/examples/shot.rs:591-650`), so it reports the *settled* response and is identical for
any easing constant — an identity, not a coincidence. Measured: `fragment_kaleido` reported bass
0.228 / mid 0.153 / treb 0.131 both before and after every constant in its `[smoothing]` table
changed, `warp` reverting from an asymmetric pair to symmetric, and `flash` gaining easing it never
had.

**2. No synthesized signal has dynamics.** Measured through Plan 0033 Phase 1's band report:

| `--signal` kind | bass min / mean / max |
|---|---|
| `bass:60` | 0.187 / 0.187 / 0.187 (zero variance) |
| `chord` | 0.058 / 0.059 / 0.060 |
| `noise:7` | 0.012 / 0.022 / 0.039 |
| `click:120` | 0.000 / 0.000 / **0.011** |

`click_track` is the only generator with real transients, and it peaks roughly 50x below the range
shipped presets are gained for.

The cost is concrete rather than theoretical. Authoring `rose_trails`, the content lane rendered five
`thickness` values from 1.10 to 2.30 — **including the untouched original** — and could not tell them
apart, because that preset's 1.25 spin against a max-decay feedback saturates any held stimulus
regardless of the value. It shipped a mid-range guess and said so in the file.

Two problems hide here and conflating them is what kept this unsolved: *does the easing behave*
(wants a controlled stimulus and a measurement) and *are gains calibrated for real material* (wants a
realistic one). This plan separates them and answers both.

## Decision

Per ADR-0039: a **deterministic transient probe** is the primary answer, because it tests the
capability directly rather than approximating the input. A **synthesized dynamic generator** follows,
so filmstrips exercise the DSP with material that rises and falls. A **committed reference clip is
rejected** — repository weight against "lightweight is a feature", plus an unanswered licensing
question — and the calibration numbers it would have supplied come instead from a `human` phase
measuring the user's own audio.

The probe measures the **frame**, not the parameter, so it is a floor on observability rather than a
guarantee: a preset whose visual response saturates still reads flat. That limitation is documented
in Phase 5 rather than left to be rediscovered.

**No CI gate.** Transient response has no fair universal floor — a slow ambient preset legitimately
has a slow rise — and `animation.rs` already shows that failure mode (backlog 0009). The number ships
as a column the content lane reads. A gate can follow once there is evidence about the range real
presets occupy.

## Architecture diagram

```mermaid
flowchart TD
    subgraph core["core/ — source-agnostic"]
        SIG["signal.rs<br/>+ dynamic generator<br/>(envelope + beat grid)"]
        DSP["Analyzer<br/>(real DSP)"]
        CAP["Renderer capture<br/>+ time-varying stimulus"]
        MET["metrics.rs<br/>+ step-response measure"]
    end
    subgraph shell["standalone/ — the shot harness"]
        SIGNAL["--signal <kind>"]
        REPORT["--report<br/>+ rise/fall column"]
        AUDIO["--audio <clip.wav><br/>(human phase)"]
    end

    SIGNAL --> SIG --> DSP --> FILM[filmstrip]
    AUDIO --> DSP
    REPORT --> CAP --> MET --> COL["rise / fall / ratio"]

    STEP["step stimulus<br/>0 -> 1 -> 0"] -.drives.-> CAP
```

## Implementation phases

Each phase ships as its own commit. Phases 1-3 and 5 are `dev`; Phase 4 is the user's.

### Phase 1 — A time-varying stimulus, and a step probe end to end
- **Owner skill:** dev
- **What:** The walking skeleton. The capture path learns to drive a stimulus that *changes*, and one
  purpose-built fixture proves a step response is measurable end to end. This is the phase that
  closes the mechanism in backlog 0013.
- **Files touched:** `core/src/render/mod.rs`, `core/src/render/metrics.rs`,
  `core/tests/fixtures/`, `core/tests/` (a new test or an existing capture suite)
- **Done when:**
  1. The capture path can render N frames whose `AnalysisFrame` varies per frame — a step from
     silence to a held stimulus and back. Whether this is a new method beside `capture_preset` or a
     generalization of it is `dev`'s call, but **the existing `capture_preset` signature and
     behaviour stay working**, since `--report`, `sanity`, `reactivity` and `animation` all consume
     it; say which shape was chosen and why in the commit body.
  2. A **purpose-built fixture** (not a shipped preset) binds one parameter with a near-linear visual
     response, so the probe measures easing rather than a scene's saturation curve. It carries the
     same "do not tune, this is a baseline" header as `core/tests/fixtures/composite_*.toml`.
  3. Two variants of that fixture — one with a **scalar** `[smoothing]` entry, one with
     `{ attack, release }` — produce **measurably different** probe results, and the test asserts the
     *property* rather than a tuned constant: the asymmetric variant's fall-time-to-settle is
     **dramatically longer** than its rise-time, while the scalar variant's two are **equal within
     tolerance**.

     The arithmetic that property rests on, so `dev` can sanity-check the measurement rather than
     tune to it: a one-pole reaches 90 % of a step in `t = tau * ln(10) = 2.303 * tau`, and decays to
     10 % in the same time. So a scalar entry has a fall/rise ratio of **exactly 1.0** by
     construction, and `{ attack = 0.02, release = 0.7 }` has a *parameter-domain* ratio of **35**
     (0.046 s up, 1.61 s down — about 3 frames against 97 at 60 Hz). The **pixel-domain** ratio will
     differ, because the scene's response is not linear; do not assert 35. Assert that the scalar
     case is ~1.0 and the asymmetric case is far from it, and **state the measured pixel-domain
     numbers in the commit body** so the next change has a reference.
  4. Verified non-vacuous: with the two fixtures' `[smoothing]` tables swapped, the test fails.

### Phase 2 — The probe becomes a `--report` column
- **Owner skill:** dev
- **What:** Makes Phase 1's measurement routine for the content lane, which is the point — a
  capability only `dev` can invoke does not close the lane's gap.
- **Files touched:** `standalone/examples/shot.rs`, `standalone/src/shot/json.rs`,
  `standalone/tests/shot_cli.rs`, `docs/capturing.md`
- **Done when:**
  1. `--report` gains a transient column (rise, fall, or their ratio — `dev` picks the presentation
     that reads best in the existing fixed-width table) computed for every preset in the loaded
     library, and `--json` carries the same values.
  2. Running it over the shipped set **separates presets that use `{ attack, release }` from those
     that do not** — at least directionally. **If it does not, that is a finding, not a failure:**
     say so with the numbers, because it would mean the probe cannot see through real presets'
     visual response and Phase 5's documented limitation is larger than expected.
  3. The added wall-clock cost of `--report` over the whole library is measured and stated. It
     already runs six captures per preset; if the probe pushes a full-library report past roughly
     double its current time, `dev` stops and surfaces it rather than absorbing it — the lane runs
     this command constantly and its cheapness is why.

### Phase 3 — A synthesized generator with musical dynamics
- **Owner skill:** dev
- **What:** Closes the stimulus half. One new generator in the source-agnostic core, so a filmstrip
  exercises the DSP with material that actually rises and falls.
- **Files touched:** `core/src/signal.rs`, `standalone/src/shot/args.rs`, `docs/capturing.md`
- **Done when:**
  1. A new `--signal` kind produces an **envelope-shaped, beat-gridded** signal with energy across
     all three bands, and is a pure function of its arguments (no wall clock; seeded randomness only,
     NFR §6) — the invariant every generator in `core/src/signal.rs` already holds.
  2. Its band levels have **real dynamics**, stated as a property because no honest threshold exists
     yet: `max / mean` per band is materially above 1, where every existing kind is at or near 1.0
     (`bass:60` is exactly 1.000, `chord` 1.017, `noise:7` 1.77). Report the measured min/mean/max
     for the new kind in the commit body, from the band report Plan 0033 Phase 1 added.
  3. `docs/capturing.md` states plainly that this kind exercises *dynamics* and is **not** evidence
     about real loopback levels — Phase 4 is what speaks to that.
  4. The existing `--signal` kinds are unchanged and their tests still pass.

### Phase 4 — Measure what real material actually produces
- **Owner skill:** human
- **What:** Closes backlog 0008 item 3 without committing a binary. Only the user has real music and
  a real playback path.
- **Done when:**
  1. The user runs `shot --audio <clip.wav> --strip 8` against one or more files of their own
     choosing — ideally a quiet track and a loud one — and captures the printed band min/mean/max.
  2. Those numbers are recorded in `docs/capturing.md` as the reference range authors calibrate
     gains against, with the material described generically (genre and rough loudness, not a
     filename) so the figures stay meaningful without shipping or naming the audio.
  3. If the measured levels differ substantially from what the shipped library is gained for, that is
     **captured as a new backlog entry**, not fixed here — re-gaining the whole set is a content-lane
     pass with its own scope.

### Phase 5 — The doc sweep
- **Owner skill:** dev
- **What:** The required operator-doc sweep, plus the two documentation-only backlog items that
  belong with it.
- **Files touched:** `docs/capturing.md`, `docs/preset-palettes.md`, `presets/README.md`
- **Done when:**
  1. `docs/capturing.md` documents the transient column and **its limitation** — that it measures the
     frame rather than the parameter, so a preset whose visual response saturates (`rose_trails` is
     the worked example) reads flat regardless of its easing.
  2. **Backlog 0012 as a clarification, not a fix.** The entry's premise was wrong and was corrected
     at promotion: `coverage` (`core/src/render/metrics.rs:69`) uses `is_lit` (`:109`), which is
     `abs_diff > eps` — a symmetric difference from the corner background, so dark-on-light and
     light-on-dark measure identically. `docs/capturing.md` gains a sentence that a low `cover` is
     **expected and correct** for a deliberately sparse or ink-remapped look (`reaction_coral_bloom`
     at 0.128 is healthy), and that the column names suspects rather than convicting them.
  3. **Backlog 0014.** `docs/preset-palettes.md` gains a swatch table for the line scenes' cosine
     `hue` ramp, which is not a hue wheel and is currently undocumented. Measured points to seed it:
     **0.06 lavender, 0.17 turquoise, 0.30 cyan, 0.46 near-white/green, 0.62 gold, 0.82 rose.** Note
     that the three line scenes ignore `[palette]` entirely, so this is their only colour control.
  4. No count-bearing sentence is introduced that will re-drift.

## Data shapes

```rust
// illustrative — not the final interface

// Phase 1: the capture path gains a stimulus that varies per frame. Shape is
// dev's call; what matters is that capture_preset's own signature survives.
// A slice indexed by frame and a closure are both reasonable:
fn capture_preset_over(
    &mut self,
    name: &str,
    stimulus: &[AnalysisFrame],   // one per frame
) -> Result<Vec<CaptureImage>, RenderError>;

// Phase 1: the measurement, in core/src/render/metrics.rs beside frame_diff.
// Frames to reach `settle_frac` of the total change, up and down.
pub struct StepResponse {
    pub rise_frames: u32,
    pub fall_frames: u32,
}
// ratio = fall/rise. Scalar [smoothing] => 1.0 by construction; an
// { attack, release } pair => far from 1.0. The pixel-domain value differs
// from the parameter-domain one (35 for 0.02/0.7) and is not asserted.
```

## Risks & open questions

- **The probe may not see through real presets' visual response.** Phase 1 uses a purpose-built
  near-linear fixture precisely so the mechanism is provable; Phase 2 done-when 2 is where we find
  out whether it survives contact with the shipped set, and it is written to make a negative result a
  *reported finding* rather than a phase failure. `rose_trails` is the known-hard case and may simply
  remain unverifiable — its saturation is real, not a measurement artifact.
- **`capture_preset`'s neighbourhood is a production API changed for testing's benefit.** ADR-0039
  accepts this explicitly. The mitigation is done-when 1's requirement that the existing signature
  keeps working, since four suites depend on it.
- **A synthesized "musical" signal risks false confidence.** Someone will read a green probe over a
  dynamic signal as "my gains are right". Phase 3 done-when 3 and Phase 5 both push back on that in
  writing; it is a documentation risk, not a technical one.
- **`--report` is the lane's most-used command and this makes it slower.** Phase 2 done-when 3 makes
  that a stop-and-surface rather than a cost absorbed silently.
- **Open:** whether the transient column should eventually become a gate. Deliberately deferred —
  ADR-0039 records why, and Phase 2's measured spread over the shipped set is the evidence that
  decision would need.

## What this plan does NOT do

- **No committed audio clip**, and no `--audio` change. ADR-0039 records the repository-weight and
  licensing reasons; Phase 4 gets the numbers without the bytes.
- **No CI gate on transient response** — see the Decision section and backlog 0009's precedent.
- **No re-gaining of the shipped presets** even if Phase 4 shows the library is calibrated for the
  wrong levels. That is a content-lane pass and gets its own scope; Phase 4 done-when 3 routes it to
  the backlog.
- **No change to `--report`'s existing columns.** Settled per-band reactivity answers a different and
  still-useful question; ADR-0039 rejects overloading it.
- **No `coverage` code change** — backlog 0012's premise was corrected at promotion and it is a
  documentation item now.
- **No C ABI change, no new dependency, no scene changes.**

## Followups (after this lands)

- Whether the transient column earns a CI gate, decided against Phase 2's measured spread.
- A content-lane re-gain pass, if Phase 4's measurements say the library is calibrated wrong.
- Backlog 0010 (the kaleidoscope fold's out-of-range clamp) and 0011 (the fold axis versus `pan_*`) —
  unrelated to this plan, still waiting, and 0010 is ADR-worthy.
