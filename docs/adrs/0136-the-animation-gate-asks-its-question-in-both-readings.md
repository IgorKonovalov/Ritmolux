# ADR-0136 — The animation gate asks its question in both readings

> **Status:** accepted 2026-08-28 (Plan 0123)
> **Date:** 2026-08-27
> **Related plan(s):** [0123](../plans/done/0123-a-gate-a-latch-and-an-ink.md)
> **Supplements:** [0091](0091-the-animation-gate-scores-motion-against-the-figures-footprint.md), [0134](0134-motion-is-two-readings-and-anchoring-is-why-neither-can-be-a-threshold.md)

## Context

`core/tests/animation.rs` captures two frames 24 apart against `AnalysisFrame::default()` and fails
any preset whose `footprint_diff` is under `ANIM_FLOOR = 0.01`. The silence is deliberate and the
module header says so: *"the motion under test is the shared scene clock, not an audio edge."* The
gate exists to catch a frozen clock — `time` unbound, a stuck accumulator — and for every world the
library held when it was written, autonomous motion and liveliness were the same property.

They stopped being the same property with `collage_mono`, a poster built to sit still and do nearly
all its moving in response to the music. It measures **0.0025**, a quarter of the floor, while
passing `reactivity` — the one gate that drives real PCM through the real analyzer — without
trouble. Two gates that both ask "is this preset dead" disagree, because one of them asks its half
in isolation.

What shipped is the workaround, and it is the kind this project has learned to distrust: the
`pan_x`/`pan_y` rates went from `0.07`/`0.09` to `0.70`/`0.78` — an 11 s sway in place of a 90 s
one — added for the measurement rather than for the picture, and the preset header says as much. The
content lane also measured that the obvious lever is a dud: `drift` and `spin` raised through
`0.55 -> 2.50` and `0.30 -> 1.60` leave the statistic at `0.002`, unchanged, because both multiply
each element's own seeded velocity and 0.4 s of that is nothing. Only whole-canvas motion moves the
number, which is precisely the motion a poster must not have.

The cohort this came from is built on stillness, so this is a structural cost rather than one
preset's bad luck: every future world whose liveliness is audio-driven has to buy motion it does not
want.

## Decision

`every_preset_animates_over_time` passes a preset that clears its floor on **either** reading — the
silent motion it measures today, or a silent-versus-driven differential — and **the sweep prints
which branch each preset passed on**, so the set of worlds that are still images in silence is a
visible, countable roster rather than an invisible consequence of an `||`.

Four things make this a decision rather than an `||`, and each has already been a defect in this
repo in some other form:

**Both readings use this gate's own statistic.** The driven branch is `footprint_diff` over a silent
and a driven capture at the same frame count, not a number imported from `reactivity.rs`. That gate
measures `frame_diff` — a whole-frame mean — against a floor of `0.02`. Disjoining two floors on two
different statistics in two different units is the
[ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) /
[ADR-0074](0074-a-ratio-against-an-in-run-control-is-not-automatically-portable.md) error
with an `||` in front of it.

**The driven capture is a synthesized fully-driven `AnalysisFrame`, not PCM.** This gate's question
is *"does the picture change"*; whether the audio path reaches the picture is `reactivity.rs`'s
question and it stays the only PCM gate, so `docs/capturing.md`'s table is unchanged by this ADR.
`standalone/src/shot/report.rs`'s `loud_frame()` is that frame today and would become the second
definition of "fully driven" in the tree; it moves into `core` so both read one.

**The driven floor is derived, not chosen.** `ANIM_FLOOR`'s own doc comment records its derivation —
half the shipped library's minimum, with the noise ceiling shown to sit below it and a non-vacuity
pair bracketing it — and the driven floor is derived the same way from the same printed sweep. A
number picked so that `collage_mono` passes is not a floor, it is a permission slip.

**A preset frozen in both readings still fails.** The disjunction weakens the gate exactly as far as
one branch, and no further: the existing static control must keep failing, on both branches, because
a scene that does not move in silence and does not move under full drive is frozen in the only sense
the gate ever meant.

## Consequences

### Positive

- **A world may be still on purpose.** The cohort that motivated this stops paying for the
  measurement in motion nobody wanted, and `collage_mono`'s sway can come back down to what the
  composition asked for.
- **Nothing new is measured.** Both readings are `footprint_diff` over captures the harness already
  knows how to take, at 96x96 on the software adapter. The cost is two more captures per preset in a
  test that already takes two.
- **One definition of "fully driven".** `loud_frame()` stops being a private helper in the
  standalone shell and becomes the thing both the gate and `--report` read, so the two cannot drift.
- **The weakening is visible.** The printed branch makes "still in silence" a property with a roster
  behind it, which is the form this project has repeatedly found it needs — an unprinted property is
  one nobody re-reads.

### Negative

- **A world that renders as a still image in silence now ships, and only a printed line says so.**
  That is a real regression in what a green suite guarantees, and it is the price. The mitigation is
  the roster, not an argument that the case does not matter.
- **A second floor to derive and re-derive.** Two constants now have to be re-derived when the
  library's minimum moves, where there was one. The sweep prints both distributions, so the
  derivation is mechanical, but it is twice the bookkeeping.
- **A preset can now pass while being frozen under one specific reading**, and the gate no longer
  distinguishes "moves on its own" from "moves only with music" *for the purpose of failing*. Anyone
  who wants that distinction reads the branch column.

### Neutral

- No change to `reactivity.rs`, to `docs/capturing.md`'s PCM table, or to `ANIM_FLOOR` itself.
- No change to `--report`: [ADR-0134](0134-motion-is-two-readings-and-anchoring-is-why-neither-can-be-a-threshold.md)'s
  `drive` column stays a printed reading with no threshold on it.

## Alternatives considered

### Alternative A — a silent disjunction, with nothing printed

The smallest change: make the verdict an `||` and stop. Rejected because it deletes the information
rather than relocating it. The fact that a world is a still image in silence would then be recorded
nowhere in the tree — not in the gate, not in the sweep, not in the preset — and this project's own
record is that a property nothing prints is a property nobody re-reads. The roster costs one column.

### Alternative B — the preset declares itself

A schema key by which a preset opts into the driven branch, making stillness an authored intent
rather than a measurement outcome. Rejected on two counts: a forgotten declaration reads as a broken
preset and sends its author to debug a working look, and the declaration is an assertion the gate
would then have to trust — where the measurement already knows the answer and cannot forget to make
it.

### Alternative C — lower `ANIM_FLOOR` until `collage_mono` passes

Rejected by ADR-0091's own arithmetic, which is the decisive reason. `collage_mono` reads `0.0025`,
and the derived noise ceiling — one pixel swinging full-scale on all three channels in an otherwise
empty frame, with the mask floored at `MIN_FOOTPRINT_FRAC` — reads `0.0072`. Any floor that passes
this preset also passes that flicker, so the gate would stop separating a live scene from a dead one
with a stuck pixel. There is no floor on this statistic that admits the preset and excludes the
noise, which is what makes the second reading necessary rather than convenient.

### Alternative D — gate on `drive`, the `--report` column ADR-0134 already added

Not so much rejected as distinguished, and it needs stating because the surface reading of ADR-0134
forbids this whole ADR. That decision says `drive` and `rate` are printed readings and never gates,
and its argument is about **ordering**: anchoring means a higher `rate` does not mean a worse preset,
so no threshold on it ranks the library, and the two presets that measure highest are both
comfortable to watch. A disjunction ranks nothing. It asks whether a preset is frozen in *both*
readings — the one question about motion this project has always been willing to gate — and answers
it with the gate's own statistic rather than with `--report`'s. What is rejected here is the literal
form: `animation.rs` does not read `--report`'s column, does not adopt its size, and introduces no
threshold on a rate.

## Outcome (2026-08-28, Plan 0123 close)

Accepted as decided. `DRIVEN_FLOOR` landed at **0.017**, half the shipped library's minimum on the
driven statistic rounded down; that minimum is **0.0345** (`Valentine`), re-measured at this close
over all **54** presets and unmoved by the preset the plan added (`Broadside` reads driven 0.4042).
The printed roster is the two worlds the disjunction exists for — `Collage Mono` (silent 0.0025 /
driven 0.0621) and `Suprematist` (0.0082 / 0.0627) — so the premise that **more than one world wanted
this** is now measured rather than argued, and on a second preset chosen by eye and not by the plan.

**One consequence this ADR did not anticipate, and it is the reason a re-derivation is owed.** Adding
a second branch changed what the phrase *"the shipped library's minimum"* means in `ANIM_FLOOR`'s own
recorded derivation, which predates this decision and still reads as though the gate had one
population. Over the 54 shipped presets there are now three defensible readings of it — the literal
minimum **0.0025** (`Collage Mono`, which passes on the other branch), the minimum among presets that
pass the **silent** branch **0.0201** (`On White`), and the **0.0205** (`Banded Mandala`) the comment
names, which is no longer the minimum of either population. The floor itself is unharmed: `0.01` sits
under 0.0201 with 2.01x slack, which is the 2.05x it claims to within the rounding. What needs
repair is the *statement* — a derivation under a disjunctive gate has to name its population.
Filed as design-backlog 0152.
