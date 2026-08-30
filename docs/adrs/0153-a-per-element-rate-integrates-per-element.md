# ADR-0153 — A per-element rate integrates per element

> **Status:** proposed
> **Date:** 2026-08-29
> **Extends:** [ADR-0132](0132-a-rate-parameter-integrates-a-phase.md)
> **Related plan(s):** [0140](../plans/0140-every-rate-integrates-for-real.md)

## Context

[ADR-0132](0132-a-rate-parameter-integrates-a-phase.md) decides that **every bindable rate parameter
in this engine integrates a phase**. ADR-0135 and Plan 0122 delivered that for the six rates measured
against `self.time`, and added a `hygiene.rs` guard that fails the build on `self.<field> * self.time`.

**Three more rates multiply a per-element `age` instead** — the same defect against a different
clock, and the guard matches the shared clock by name, so it passes all three:

```
shape_collage.rs:1320  center:    p.spec.center    + p.vel * drift * age
shape_collage.rs:1321
shape_collage.rs:1324  angle_deg: p.spec.angle_deg + (p.spin * spin * age)
emitter.rs:776         sprite:    base + rate * age
```

`drift` and `spin` are both in `shape_collage`'s `PARAMS` roster, so both are bindable, and **a
binding that moves retroactively rescales every second of the element's life.**

**Three shipped presets bind the collage pair to audio** — more content than the `swarm` pair Plan
0122 existed to fix:

```
collage_onwhite.toml:108-109      drift bass swing 0.4   spin mid swing 0.35   [smoothing] 0.6
collage_suprematist.toml:116-117  drift bass swing 0.6   spin mid swing 0.5    [smoothing] 0.6
collage_mono.toml:43-44           drift bass swing 0.60  spin mid swing 0.30   [smoothing] 0.60
```

Size, computed rather than measured: a one-pole at `tau = 0.6` closes 2.74 % of its gap per 60 Hz
frame, so `collage_suprematist`'s `spin` moves 0.0137 in a frame across its 0.5 swing. With
`SPIN_SPEED = 0.07` the angle jumps `0.07 · 0.0137 · age` — at `age = 30 s` that is 0.029 rad in one
frame against a nominal 0.00058, about **49x**; `drift` is ~35x by the same route.

**Milder than `swarm`'s 210x, and bounded differently.** `age` resets on each `recompose`, where
`swarm`'s `time` never resets, so in normal playback this stays a jitter rather than a teleport. The
exception is the quiet passage: `recompose` is gated on `hash(beat_index)`, so with no onsets it
never fires, `age` grows unbounded, and **the first bass hit after it lands the full accumulated
swing.**

The decision is forced by a second fact. `presets/README.md:1536` documents the defective form as the
safe one — *"Integrated against real elapsed time, so the canvas moves identically at any refresh
rate"* — which is true about frame-rate independence and false about ADR-0132, and it is how three
presets came to bind it.

## Decision

**A per-element rate integrates into per-element state, advanced with the element and reset when the
element is born or the canvas recomposes.** `shape_collage`'s `drift` and `spin` each gain a
per-element accumulator; the placement becomes `spec + accumulated` rather than `spec + rate · age`.

This is deliberately **not** `scenes::Phase`, and that is the whole reason Plan 0122 scoped these out
rather than folding them in: `Phase` is one accumulator per scene, and these need one per element.
The shape is analogous, the storage is not, and forcing them into `Phase` would either allocate a
`Phase` per element or pretend a shared one is per-element.

**`emitter`'s `spin` is measured before it is repaired.** Sprites are short-lived, so `age` is small
and the defect may be unobservable; if the measurement says so, it is documented as bounded-by-
lifetime rather than converted. ADR-0132 corrected `parametric_curve` and `warp_mesh`'s rates when
they were bound only to constants, and that precedent is why this is measured rather than skipped —
but a rate whose clock cannot grow is a different fact from a rate nobody happens to bind.

**The `hygiene.rs` guard is not widened to `* age`.** `emitter.rs:375-376`'s `v0 * age` ballistics
are **legitimate** — the velocity is baked at spawn, so the multiply is a position, not an
integration — and a naive widening false-positives on correct code. The guard keeps its current
spelling; what replaces the missing coverage is that the three sites are repaired and
`presets/README.md`'s row stops teaching the defective form.

## Consequences

### Positive
- A moving binding steers the canvas from now on rather than rewriting the element's history, which
  is what an author binding `drift` to bass reasonably expects.
- The quiet-passage cliff goes: no accumulated swing waiting to land on the first bass hit after a
  long gap without onsets.
- ADR-0132's rule becomes true of the engine rather than true of the rates someone grepped for.
- The three shipped collage presets keep their bindings and their intent; only the integration
  changes.

### Negative
- **Per-element accumulators cost memory and a write per element per frame**, where the current form
  is a multiply at read time. `shape_collage`'s element counts are small, but this is a real
  regression in a hot loop and must be measured, not assumed.
- **It changes what the three shipped presets look like**, and their `[smoothing]` values were tuned
  against the defective response. Goldens will move and the presets will want a retune — content-lane
  work that this decision creates.
- **The guard still cannot see this class.** Declining to widen `hygiene.rs` is the honest call given
  the legitimate `v0 * age` case, but it means the next per-element rate added has nothing catching
  it except review — which is exactly how these three arrived.
- `emitter`'s third case may be left as a documented non-repair, so the rule has a stated exception
  from day one.

### Neutral
- No change to `Phase`, to the six rates Plan 0122 already fixed, or to the shared clock.

## Alternatives considered

### Alternative A — Bake the rate at spawn, the way `emitter` already bakes `v0`
Cheapest by a wide margin: one read at element birth, no per-frame accumulator, no memory growth, and
the ballistics precedent already exists in the same file. Rejected because **it changes what the
parameter means** — the rate stops steering the live canvas and becomes a property of elements
created after the binding moved, so an author binding `spin` to mid hears the music affect only new
elements. For a scene whose elements are long-lived and whose recompose is onset-gated, that is close
to no reactivity at all in exactly the passages where reactivity is wanted.

### Alternative B — Widen the `hygiene.rs` guard to `* age` and repair whatever it convicts
Attractive because it turns the rule into something mechanical, which is this project's strong
preference. Rejected on false positives: `emitter.rs:375-376`'s `v0 * age` is correct ballistics with
a spawn-baked velocity, and a guard that must be suppressed at a legitimate site teaches its readers
to suppress it. A guard with an allowlist is the shape ADR-0058 already warns against.

### Alternative C — Do nothing; the magnitude is 4-6x smaller than what Plan 0122 fixed
The null option and not unreasonable: `age` resets bound it in ordinary playback, nothing has been
reported, and the priority is Medium. Rejected because the **documentation actively teaches the
defective form**, so the population of affected presets grows on its own — three already, against
`swarm`'s two — and because the quiet-passage case is not bounded at all.

## Notes

Discharges [design-backlog 0149](../design-backlog.md). The `presets/README.md:1536` correction is
worth making **whether or not the engine repair is taken**, costs one sentence, and stops the content
lane writing more of these — it is the first phase of Plan 0140 rather than a consequence of this
decision.
