# ADR-0042 — Preset reachability is measured on the expression tree, not inferred from frames; `--report` reads at two levels

> **Status:** accepted
> **Date:** 2026-07-28
> **Related plan(s):** [0041](../plans/done/0041-report-two-level-stimuli-and-expression-reachability.md)
> **Supplements:** [ADR-0036](0036-preset-reachable-spectrum.md) (the band axis this measures against),
> [ADR-0039](0039-verify-easing-with-a-transient-probe-not-a-committed-clip.md) (the stimulus policy this extends)

## Context

Every measurement in this repo's visual-QA harness is a **frame differential**. `--report`'s
reactivity columns render a preset under a held stimulus, render it again under a band-lit stimulus,
and diff the pixels; `anim` diffs two frames in time; `cover` counts pixels differing from the
backdrop. That has served well — but it has now failed silently, twice, in two different ways, and
the two failures share one cause.

**The first failure is a whole class of dead presets.** The `preset-author` lane's 2026-07-28 library
audit (design-backlog
[0020](../design-backlog.md))
found that comparison gates across the shipped set were written against `--set bass=1` magnitudes
rather than measured audio. Measured through `shot --signal dynamic:110`, bass means `0.040` and mid
and treb mean `0.006`; the three-band sum peaks near `0.157`. Against that, thresholds like
`bass + mid + treb > 0.90`, `bass + mid > 0.42` and `mid > 0.22` **never evaluate true**. Six shipped
presets had their headline mechanism disabled outright — `fragment_kaleido` sat at 6 folds for the
program's entire life, never once running the audio-driven symmetry it exists to demonstrate;
`reaction_reef`, the family's designated *figure*, never folded; `lsystem_arrowhead` never subdivided
past its coarsest depth. Three more (`attractor_dejong`, `attractor_lorenz`, `fragment_warp`) are
still dead today.

**`--report` scored every one of them as healthy.** Its band stimuli set the scalar to `1.0`
(`band_stimulus`, `standalone/examples/shot.rs`), so at full scale every one of those gates fires
happily and the differential columns record a lively preset. The instrument all three lanes
self-verify through is the one thing that could not see the defect.

**The second failure is design-backlog [0022](../design-backlog.md)**,
and it has the same root. `curve` is `level^curve`, and at a level of `1.0` that is the identity at
any exponent — so a full-scale stimulus is *mathematically incapable* of seeing the compression a
level curve buys. A preset could ship `curve = 0.05` and the report would show nothing but the
`scale` cut that paid for it. 0022 recorded that it becomes ADR-worthy "only if bundled with 0020
into a decision about what level the report's stimuli should represent". This is that bundle.

So there are two questions, and they are not the same question. *What level should the stimuli
represent* is about the frame differentials. *Is this branch reachable at all* is a property of the
expression, and no stimulus level answers it — a gate that fires under every stimulus we try is still
only "not observed dead", and one that fires under none may simply want material we did not
synthesize.

## Decision

We will make two changes, deliberately at different layers.

**`--report` keeps its full-scale stimuli and gains a second reading at realistic levels**, reported
as additional columns beside the existing ones rather than replacing them. The existing columns keep
their meaning, so every number recorded in prior commits and backlog entries stays comparable. The
**gap between the two readings is the new signal**: a binding gated on an unreachable threshold moves
at full scale and not at realistic level, and a level curve compresses at realistic level and not at
full scale. Both previously-invisible defects become a difference between two columns.

**Reachability is measured on the expression tree, not inferred from pixels.** `core/src/preset/expr.rs`
gains an opt-in *probed* evaluation entry point that records, per AST node across a run: for each
`select()`, whether its condition ever took both values; for each `clamp()`, whether its upper bound
was ever approached. This is a **structural** measurement — the first in the harness that is not a
frame differential — and it is therefore immune to stimulus level in a way no column can be. It is a
separate entry point from `Expr::eval`, never reached from the render path, so the per-frame hot path
pays nothing.

The detector is **advisory output only** in this plan. It becomes a behavioral gate alongside
`sanity`/`reactivity`/`animation` only after the un-swept presets are re-gained — gating first would
land CI red for everyone on day one.

Stimuli stay **synthesized**: `--signal dynamic:<bpm>` (Plan 0037) supplies the realistic-dynamics
generator, and ADR-0039's rejection of a committed reference clip — repository weight against
"lightweight is a feature", plus an unanswered licensing question — is untouched and still holds.

## Consequences

### Positive

- **The two defects that hid from the harness become visible**, by different mechanisms suited to
  each: dead gates structurally, curve compression as a column gap.
- **Non-breaking.** Existing columns, existing numbers, existing reading habits all survive. A
  reader who ignores the new columns loses nothing they had.
- **Reachability is stimulus-independent**, so it does not inherit the "is this level realistic
  enough?" argument that the columns will always carry.
- **The un-swept library sweep becomes verifiable.** The ~10 presets still carrying dead gates can be
  fixed and then *confirmed* fixed, rather than hand-checked by arithmetic as this session's pass was.

### Negative

- **A second evaluation path can diverge from the real one.** `eval_probed` must produce identical
  values to `Expr::eval` or the report describes a preset that does not exist. This is the main risk
  and Phase 2 pins it with a test asserting the two agree across the shipped library.
- **"Never fired" means "never fired under this stimulus", not "unreachable".** The output must be
  worded as a suspect, not a conviction — the same discipline `--report`'s existing columns already
  carry ("the numbers name the suspects; the stills tell you which failure it is"). A preset
  legitimately gated on material the generator does not contain will be flagged and will be a false
  positive.
- **The table gets wider**, and it is already dense. Nine columns become thirteen-ish. Phase 3 has to
  make a layout call, and a wide terminal is not guaranteed.
- **The realistic level is a judgment baked into the harness.** Picking "typical" from one generator
  at one BPM is a choice that will be wrong for some material, and it will quietly become the number
  every future preset is gained against — exactly the role `--set bass=0.8` played in causing this.

### Neutral

- The low-level stimulus is derived from `--signal dynamic:110`'s measured output rather than being a
  new signal kind, so it adds no generator surface.
- `bin()`-reading and `spectrum` presets get the same treatment for free, since the low-level
  stimulus lights the band array proportionally exactly as `band_stimulus` already does.

## Alternatives considered

### Alternative A — Replace the full-scale stimuli with realistic ones

One set of columns, measured at realistic levels. Simplest table and it measures what presets
actually do. **Rejected because it silently redefines every historical number.** Backlog entries,
commit messages and ADR Outcome sections quote `--report` figures throughout — 0022 quotes
`bass 0.084 → 0.068`, 0013 quotes `Kaleido Field bass 0.228` — and none of them would mean what they
say any more, with nothing in the output to indicate the change. Realistic levels are also not
sample-rate-independent (design-backlog 0015: below the band-axis crossover the mapping moves with
the sample rate), so the replacement would be *less* reproducible than what it replaced.

### Alternative B — Infer dead gates from the frame differentials alone

Detect a dead branch by rendering at two levels and looking for a preset whose structure never
changes. **Rejected because the frame cannot distinguish "the gate never fired" from "both branches
look similar".** A `select(cond, 6, 8)` on fold order and a `select(cond, 6, 6)` produce identical
differentials, and so does a gate that fires correctly into a branch that happens to render alike.
The question is about the expression, so it should be asked of the expression — and asking the AST
also yields *which* gate, which a differential never could.

### Alternative C — Gate it in CI immediately

Add the behavioral test in the same plan and fix the presets to make it pass. **Rejected on
sequencing.** At least nine shipped presets would fail on day one, so the gate lands red and blocks
everyone until an unrelated content pass completes. Backlog
[0009](../design-backlog.md)
already set the precedent that a harness measurement earns a gate only once there is a fair floor and
a clean library; this has neither yet. Advisory now, gate later, is the same path
`reactivity`/`animation` took.

### Alternative D — A committed reference audio clip

Measure against real music rather than a generator. **Rejected, and this is a re-affirmation rather
than a fresh decision** — [ADR-0039](0039-verify-easing-with-a-transient-probe-not-a-committed-clip.md)
refused it on repository weight ("lightweight is a feature", NFR 4) and an unanswered licensing
question, recording the measured levels from a `human` phase instead. Nothing in this ADR changes
those forces. `--audio` remains available to a user pointing at their own file.

## Notes

The measured levels this ADR reasons from are in
[`docs/capturing.md`](../capturing.md#what-real-material-actually-produces) (real material, via
`--audio`) and reproduced by `shot --signal dynamic:110`, which prints them.

## Outcome (2026-07-29, after Plan 0041)

**The main negative did not materialize, because the implementation removed it rather than tested
around it.** "A second evaluation path can diverge from the real one" assumed two copies of the
arithmetic. There is one: the probe walk in `core/src/preset/expr.rs` *only records*, and
`Expr::eval_probed` returns `Expr::eval`'s own value. Divergence is unrepresentable, and the equality
assertion over the shipped library that this ADR asked for survives as a regression guard rather than
as the load-bearing proof. The price is that a `select()` condition and a `clamp()`'s arguments are
evaluated twice per probed call, which is free on a pure grammar nothing but the harness calls.

**The layout call went to a second block, not a wider table.** Nine columns stayed nine; the
realistic reading is its own block under each family, so every number a previous run printed is still
in the same place. The "thirteen-ish columns" this ADR worried about never happened.

**The clamp half is noisier than the `select()` half by an order of magnitude.** Over the 32 shipped
presets the probe reports **20 dead branches and 159 unapproached ceilings** — nearly every `clamp()`
in the library was written as a ceiling for full-scale input, which is the same mis-gaining. `--report`
answers with a count plus a per-family worst-three line and puts all of them in `--json`. Worth knowing
before the follow-on gate is designed: the two findings need different floors, and the ceiling check
currently flags on strict `< 1.0` rather than on "approached", so a bound reached at 99 % still counts.

**The false positive this ADR predicted is real, measured and standing.** `tempo` gates account for
**14 of the 20** dead branches — `swarm_storm` (7), `attractor_lorenz` (6) and `rose_zoom` (1) — leaving
six genuine band gates: `fragment_warp` (2), `lsystem_fern` (2), `attractor_dejong` and `star_rosette`.
The report's wording carries the suspect-not-conviction discipline this ADR required, and names the
`tempo` case explicitly as the standing exception.

Worth stating for whoever writes the follow-on gate: the honest floor for a reachability gate is
almost certainly **not** "every gate must fire". A preset gated on `tempo > 132` is correctly dead
under a 110 BPM generator, and that is a false positive by construction. The gate probably wants to
exempt gates whose condition reads `tempo`, or to run the detector across several generator BPMs, and
that is a decision for the plan that adds it — not this one.
