# 0180 — A mathematical world joins a system as a family, and a structural parameter is quantized and held

> **Status:** accepted 2026-09-10 (Plan 0161 built rules 2 and 4; rules 1 and 3 await Plans 0162-0164) - carries an Outcome
> **Date:** 2026-09-09
> **Related plan(s):** [0161](../plans/done/0161-the-structural-parameter-is-held.md),
> [0162](../plans/0162-the-curve-families.md),
> [0163](../plans/0163-the-analytic-field.md),
> [0164](../plans/0164-the-cellular-system.md)
> **Builds on:** [ADR-0007](0007-line-geometry-generators.md) (the `CurveFamily` seam),
> [ADR-0012](0012-stateful-feedback-render-system.md) (`PingPongField`, the automata's host),
> [ADR-0015](0015-gpu-compute-particle-idiom.md) (the four render idioms this wave is placed against),
> [ADR-0021](0021-shared-palette-system.md) (the colour surface every new family inherits),
> [ADR-0045](0045-quality-tiers-floor-and-rich.md) (where an iteration budget lives),
> [ADR-0046](0046-linear-light-hdr-composite-bloom-tonemap.md) (the pipeline an escape-time glow needs),
> [ADR-0170](0170-a-parameters-reference-row-is-generated-from-the-declaration-the-engine-reads.md)
> (the reference that prints what this ADR groups)
> **Relates to:** [ADR-0029](0029-parametric-curve-shape-params.md), which rejected the superformula
> as "more than the routed need" — a premise
> [`roadmap-visual-richness.md`](../roadmap-visual-richness.md) already replaced and this ADR spends.

## Context

The user asked for more of the engine's worlds to be built out of mathematics — naming fractals and
Game of Life — and a survey says the coverage is lopsided rather than thin.

**What the twelve systems already carry** (`core/src/preset/schema/system.rs:9`): chaotic maps and
flows plus IFS fractals, all inside `attractor` (`particles/family.rs:53`,
`particles/ifs.rs:290` — Clifford, De Jong, Thomas, Lorenz, and fern/tree/dragon/sierpinski/spiral);
one reaction-diffusion PDE (`reaction_diffusion`); one formal grammar (`lsystem`); one Euclidean
tiling construction (`star_pattern`); signed-distance geometry (`shape_field`, `shape_collage`); and
one hardcoded five-iteration sine fold (`fragment_field`).

**What is absent**: escape-time fractals, cellular automata of any kind, cymatics, aperiodic order,
hyperbolic geometry, growth processes, and every parametric curve family but one. Three of those
absences are already written down and unbuilt —
[`roadmap-visual-richness.md`](../roadmap-visual-richness.md)'s R4 items 1 and 2, and its reference
table's *"fractal glowing spiral … have today: neither"* row. Curve families have been
[`generative-techniques-catalogue.md`](../generative-techniques-catalogue.md)'s number-one
payoff-per-effort item since 2026-07-25, and `CurveFamily` still has exactly one variant
(`core/src/render/scenes/lines/mod.rs:246`).

So the question is not whether to add mathematics. It is what shape the additions take, because two
forces make the obvious answer wrong.

**The first force is the roster.** Adding one `SystemKind` per formula would take this wave alone to
nineteen systems. `SystemKind` is the most-copied list in the repository: the enum, `TABLE`'s
exhaustive match, `docs/presets.md`'s system table, the generated parameter reference, and the
distinctness gate's family map all enumerate it. The engine has already refused this shape once —
`attractor` holds five map families *and* five IFS figures behind one variant, and
`particles/family.rs:47` states the reason in its own words: a preset picks a figure by name rather
than through a `family` + `figure` pair.

**The second force is that this engine cannot bind a discrete parameter, and two shipped presets
prove it.** `parametric.rs:341` declares `n` as an `f32` with a documented integer meaning ("the
rose's petal number") and a `1..24` range; the same is true of `d`, `samples`, `points`,
`palette_steps` and `kaleido_order`. An author who binds one of these to audio the natural way —
`n = "3 + floor(bass * 5)"` — re-picks the figure **every frame**, rebuilding the curve at frame
rate. The result reads as flicker, not as music.

Across all 84 shipped presets, every structural integer is a constant string except two, and both
are the same hand-rolled workaround:

```toml
# curve_nightbloom.toml:49 — a three-value sample-and-hold, built by hand
d = "select(hash(floor(beat_index / 4)) < 0.33, 29, select(hash(floor(beat_index / 4)) < 0.66, 37, 43))"

# fragment_supernova.toml:44 — a three-step ladder on band energy
kaleido_order = "select(bass + treb > 1.38, 16, select(bass + treb > 0.80, 8, 3))"
```

Both are limited to three values because a nested `select` is what the surface offers. The grammar is
stateless by rule, and the one existing escape from that rule — [`[latch]`](../presets.md) — fires an
*event*; it does not hold a *value*. Every candidate in this wave makes the gap worse: Chladni's two
integer mode numbers, an automaton's rule index and reseed, an escape-time iteration budget, and the
integer counts of four new curve families all want to change **on a musical edge and at no other
time**.

Two smaller forces close the set. Iterated per-pixel mathematics diverges across adapters — the
catalogue's own caveat says so, and `golden.rs` calls a `0.02` mean channel difference rasterizer
drift, so a deep-zoom escape-time image has no stable baseline. And
[`design-backlog 0030`](../design-backlog.md) measured that presets binding audio to **geometry**
score 2-4x better on the animation metric than presets binding it to brightness; the user asked for
both surfaces, named apart, rather than one or the other.

## Decision

We will grow the engine's mathematics under four rules.

**1 — Placement.** A new mathematical world joins an **existing system as a named family** when it
shares that system's render idiom, state model and colour surface; it founds a new `SystemKind` only
when it needs its own state, its own pass shape, or a parameter surface that would be inert on the
host. Applied to this wave: the new parametric curves are `CurveFamily` arms on the existing
`parametric_curve`; the stateless closed-form per-pixel worlds (escape-time, Chladni, and later
quasicrystal, Voronoi and hyperbolic tiling) are families of **one new system, `analytic_field`**;
the automata are families of **one new system, `cellular`**. Two new systems for seven new worlds,
not seven.

**2 — A structural parameter is declared, quantized, and holdable.** `ParamSpec` gains a `kind`:
`Modal` (continuous — every parameter that exists today, smoothed as now) or `Structural` (integer
meaning — rounded once, CPU-side, before the scene sees it, which is what `palette_steps` already
does ad hoc). Independently and orthogonally, a preset may declare a **`[hold]` table**, per binding,
in the shape `[smoothing]` already uses:

```toml
[hold]
n = "bar"      # re-sample on the downbeat; hold between
d = "beat"
zoom = "2.0"   # or a bare number of seconds
```

A held binding is evaluated every frame as now, and the value the scene receives changes only when
the named edge fires — `beat`, `bar`, or a period in seconds. `[hold]` applies to **any**
bindable parameter, not only structural ones; `kind` governs quantization and documentation, not
holdability. Naming a parameter the preset does not bind is a load error, in ADR-0020's posture.

**3 — Golden regime.** A scene whose output depends on an iteration count is baselined against a
golden preset authored inside its **stable regime** — bounded iterations, shallow zoom, no sampling
near a boundary the iteration count decides. Any claim that cannot be held there is asserted as a
structural statistic rather than a pixel mean, per
[ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md). Deep-zoom and
long-horizon looks ship as presets and are covered by the reactivity and animation gates, which do
not compare pixels.

**4 — The reference names both surfaces.** The generated parameter reference
([ADR-0170](0170-a-parameters-reference-row-is-generated-from-the-declaration-the-engine-reads.md))
prints each system's parameters in two groups — **Structural** (what is drawn: modes, rules, counts,
families) and **Modal** (how it looks: brightness, colour, scale, rate) — and, for a family-bearing
system, names the family each structural parameter reads on, so a parameter that is inert for the
chosen family is visibly inert rather than mysteriously dead.

## Consequences

### Positive
- **Seven new mathematical worlds cost two roster entries.** The five lists that enumerate
  `SystemKind` grow by two, once, and every later family — Lenia, Penrose, Kleinian, Apollonian,
  sandpile — costs a `match` arm and a documentation row rather than a five-file edit.
- **The discrete-binding gap closes for the whole engine, not for this wave.** `[hold]` reaches
  `n`, `d`, `samples`, `points`, `elements`, `palette_steps` and `kaleido_order` — every one of
  which is bindable today and effectively unbindable in practice. The two hand-rolled staircases
  above become one line each.
- **Both audio surfaces become nameable.** An author can see, in the generated reference, which
  parameters change the geometry and which change the light, which is exactly the distinction
  `design-backlog 0030` measured and could not previously point at.
- **Escape-time finally has a pipeline to land in.** The HDR composite, bloom and tonemap that a
  glowing fractal needs landed with ADR-0046 and have never had a scene built for them.

### Negative
- **`analytic_field` and `cellular` each carry family-inert parameters.** Rule 4 makes the inertness
  visible; it does not remove it. An author reading `analytic_field`'s table sees rows that apply to
  one family only. This is the price of rule 1 and it is paid in documentation, permanently.
- **`[hold]` is the second part of the preset surface that depends on frame history.** `[latch]` is
  the first, and `docs/presets.md` already records that the static reachability walk cannot see
  through it. A held binding is worse in one specific way: the analyzer sees a live expression whose
  value the engine may not have taken this frame, so a preset can now appear more reactive than it
  is. `--report` will need to learn about `[hold]` or it will over-credit.
- **`ParamSpec.kind` touches every scene's `PARAMS` block** — twelve systems, mechanical but wide —
  and `declared_params_match_set_param` in `core/tests/preset.rs` does not check kinds, so it would
  pass a wrong one silently. The guard has to be extended in the same phase or the field is
  unenforced.
- **Marking an existing parameter `Structural` can move a golden.** Any parameter that currently
  takes a fractional value and would begin rounding changes its scene's output. The audit is a
  phase of its own in Plan 0161, and the honest outcome may be that some parameters stay `Modal`
  despite an integer-sounding name.
- **`bar` is only as good as the downbeat estimator, which locks ~3 % of audible time**
  ([backlog 0042](../design-backlog.md)). `[hold] n = "bar"` therefore steps on a counter-derived
  fallback on most material rather than on a tracked downbeat. That is a real limit on the most
  musically interesting edge in the vocabulary, it is not this ADR's to fix, and an author reading
  "hold on the bar" will not guess it — so the documentation has to say it at the table.
- **Two new systems double nothing but widen everything a little**: the distinctness gate's family
  map, the preset seeding roster, the on-device checklist, and the number of scenes a tier
  calibration has to cover.

### Neutral
- `fragment_field` is untouched. Its five-iteration fold stays exactly what it is, and the thirteen
  presets that name it are unaffected — see Alternative B.
- Lenia, quasicrystal, Voronoi and hyperbolic tiling are **placed** by this ADR and **not built**
  by the four plans it frames. They are family arms on systems that will exist, which is the whole
  point of rule 1.

## Alternatives considered

### Alternative A — one `SystemKind` per formula
The obvious shape: `escape_field`, `chladni`, `life`, `lenia`, `voronoi`, `quasicrystal`,
`hyperbolic`. Rejected because `SystemKind` is enumerated in five places and each addition is a
five-file edit plus a documentation row, and because the engine already refused this shape for
exactly this reason — `attractor` carries ten distinct figures behind one variant. Seven new systems
for one wave would make the roster the dominant cost of every future mathematical world.

### Alternative B — make `fragment_field` polymorphic and host the analytic families there
Superficially cheaper: it is already the fullscreen-fragment idiom with the palette LUT wired up.
Rejected because its parameter surface **is** the sine fold's own — `warp`, `fold_speed`, and an
integrated fold phase in its uniform (ADR-0132) — and its shader is a fixed five-iteration fold with
no family switch anywhere in it. Making it conditional puts the table and the generated reference of
a shipped system with thirteen presets at risk for a capability none of them use. The inertness has
to live somewhere; a new system is where it costs least.

### Alternative C — a `sample_hold(x, trigger)` function in the expression grammar
The smallest-looking fix for the discrete-binding gap. Rejected because the grammar is stateless by
rule, and a stateful function inside it would make every expression's value depend on how often and
in what order it is evaluated — which is not an invariant this engine holds, since a binding may be
evaluated per element or per vertex. `[latch]`'s precedent is the settled answer: frame history
lives in a **table**, where it is declared, counted and bounded, not in the grammar.

### Alternative D — leave discrete binding to authors
The status quo. Rejected on the evidence: two presets already built sample-and-hold by hand out of
nested `select` over a hardcoded three-value menu, and eighty-two others simply pinned their
structural integers to constants. A surface that every author works around the same way, or gives up
on, is the surface telling us what it is missing.

## Notes

The survey behind the Context section is
[`generative-techniques-catalogue.md`](../generative-techniques-catalogue.md) plus a direct read of
`core/src/preset/schema/system.rs`, `core/src/render/scenes/lines/mod.rs`,
`core/src/render/scenes/particles/family.rs` and `core/src/render/scenes/particles/ifs.rs`. The
absence list was checked against the catalogue's own "coverage gap" caveat, which already records
that no verified source survived for discrete Game-of-Life CA — a statement about the research pass,
not about feasibility.

The catalogue's pick order ranked these candidates 1 (curve families), 2 (Chladni), 3 (fractal
flames on the compute path) and 4 (Lenia). This wave takes 1 and 2, adds escape-time and discrete
automata, and leaves 3 and 4 as placed-but-unbuilt families under rules 1 and 2.

## Outcome (2026-09-10, Plan 0161's close)

Rules 2 and 4 are built. Three things the implementation established that the Decision above does
not say, recorded here rather than by editing it.

**Rule 2's last sentence contradicts itself, and the implementation followed the posture over the
severity.** It reads *"Naming a parameter the preset does not bind is a load error, in ADR-0020's
posture."* ADR-0020's posture for a name the preset does not consume is a **warning that keeps the
preset** - the opposite severity. Plan 0161 Phase 1 shipped the warning, in `[occupancy] exempt`'s
exact shape, and said so in its log. That is the right reading: the clause names one authority and
one severity, they disagree, and the authority is the half that carries a reason. **A `[hold]` entry
naming an unbound parameter warns.** What *is* a load error is narrower and was decided by the plan
rather than here: an unknown edge word, a non-positive or non-finite period, and an entry naming a
per-element or per-vertex binding - the last because an easing constant degrades to instant and a
hold has no degraded form.

**Rule 4's second half had nothing to apply to.** It asks that a family-bearing system name the
family each structural parameter reads on. No structural parameter in the engine is family-specific
today - `attractor`'s only one is `tuple`, which every family answers - so the clause is built but
unexercised. It bites when Plans 0162 through 0164 land.

**The audit rule that decided rule 2's roster is narrower than rule 2's own wording, deliberately.**
Rule 2 says `Structural` means "integer meaning". Plan 0161 Phase 3 marked `Structural` **only**
where the scene already clamps and rounds the value itself, so the engine's rounding composes to the
identity and no mark could move a pixel. That kept the phase safe - no golden moved across 27 rows -
and it left two parameters with integer meaning declared `Modal` because nothing rounds them:
`deposit_arms` (which tears along `atan2`'s branch cut at a fractional value; filed as design-backlog
0198) and `n`, the ADR's own headline example, which `curves.rs` evaluates as a raw frequency where a
fractional value is a well-defined open web rather than a broken rose. **The headline example still
works**: `n = "3 + floor(bass * 5)"` carries the author's own `floor`.

One consequence of that narrowness is that `ParamKind::quantize` is currently a no-op on every row
it reaches, so no test can distinguish it working from it being absent - design-backlog 0197.
