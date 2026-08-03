# ADR-0066 — A reseed disturbs the cloud rather than replacing it

> **Status:** proposed
> **Date:** 2026-08-03
> **Related plan(s):** [0057-the-attractors-compute-path](../plans/0057-the-attractors-compute-path.md) (Phase 3)

## Context

The attractor's `reseed` binding does not scatter the cloud — it **replaces** it. `seed_box`
returns axis-aligned half-extents per family (`±1.5` for De Jong and Clifford, `±4.5` for Thomas,
`±(20, 26, 24)` about `z = 25` for Lorenz) and `seed()` fills every particle uniformly inside that
volume (`core/src/render/scenes/particles/mod.rs:185-193` and `:1143-1155`). The cloud is then a
uniform rectangle until the map has iterated enough times to pull the points back onto the
attractor. What the user reported on `attractor_ink` is that rectangle's interior: flat random
speckle with hard axis-aligned edges, "a square artifact that breaks the flow", on **all**
attractors.

The geometry corroborates the mechanism. The box is square in world space and the vertex shader
divides `x` by the target's aspect, so on a 16:9 display it must project taller than wide — the
proportion in the report. Nothing else in the pipeline can leave an axis-aligned edge inside the
frame: the trail grid rounds up and presents as a normalized stretch ([ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md)).

**Why it is newly visible is the part that matters for how we weigh it.** The reseed gates were
*dead* for most of this project's life — every attractor shipped with `reseed` written against raw
levels it could not reach, `attractor_clifford.toml`'s own header saying it "never fired once".
Plan 0041's content re-gain (`e9a1c3c`) made them fire for the first time and Plan 0048's retune
rescaled them onto the normalized axis, so the artifact is **as old as the scene and as new as the
gate working**. `Rich` then triples the particle count into the same rectangle
([backlog 0031](../design-backlog.md)).

Two constraints shape the choice. Every shipped preset header describes `reseed` as a percussive
accent, not a wipe — so the current behavior does not match what any content author asked for. And
nothing in the harness can render the offending frame: `--set` holds a level constant, so a held
`onset = 1` reseeds every frame and averages into no visible box, and `--signal click:120`'s onset
never clears the shipped gates (the highest is `attractor_clifford`'s `onset > 0.75`). That gap is
Plan 0057 Phase 1's second instrument.

## Decision

A reseed **perturbs the existing particle positions by a bounded, family-relative jitter** instead
of re-filling the seed box. The points stay on the attractor, so a reseed reads as the figure being
disturbed rather than erased, and no uniform rectangle exists at any tier or at any moment.
`seed_box` survives and is used exactly where it is correct — the initial fill and a family change,
where there is no existing cloud to disturb. The jitter is deterministic given the per-particle
seed, so the point cloud remains a pure function of its seed and step sequence
(`particles/mod.rs:20`).

## Consequences

### Positive
- The axis-aligned rectangle is gone at its cause, and with it the convergence transient that
  followed it — which is the half of [backlog 0031](../design-backlog.md) that `Rich` makes worse,
  since three times the particles are three times the speckle in the same box.
- The behavior finally matches what six shipped preset headers already claim reseed is.
- No cost at `Rich`, no per-frame blend state, no extra pass: the change is inside the seed path
  that already runs on the reseed edge.

### Negative
- **A reseed is now less visible than today**, because a disturbance is a smaller event than a
  wipe. A preset leaning on the wipe as a structural beat loses that. The jitter magnitude is the
  lever if the disturbance reads as too subtle; returning to the box is not.
- **The cloud no longer re-randomizes.** A full re-fill re-sampled the attractor's basin every
  time; jittering in place does not, so over a long session the population stays the population it
  converged to. Accepted: the map's own mixing is what explores the attractor, and a chaotic flow
  separates jittered neighbours within a few iterations anyway.
- The initial fill still uses the box, so first-frame convergence is unchanged. For Lorenz that is
  a live question with its own phase in the plan; this ADR does not touch it.

## Alternatives considered

### Alternative A — shape the seed volume (a disc or gaussian instead of a box)
A few lines in `seed`, and it removes the **hard edge**, which is what makes the transient read as
an artifact rather than as texture. Rejected because it keeps the wipe and therefore keeps the
convergence transient — the half that gets worse at `Rich` and the half that contradicts what the
presets say reseed is. It fixes the shape of the complaint rather than the complaint.

### Alternative B — fade the reseed in over N frames
Cheapest, and it targets [backlog 0031](../design-backlog.md)'s "opaque at `Rich`" wording
directly. Rejected because it leaves a rectangle, just a fainter one, and buys that with per-frame
blend state on the reseed path — paying complexity to make a defect less legible rather than to
remove it.

### Alternative C — expose the treatment to content as a `[particles]` key
Rejected on the same ground as [ADR-0065](0065-the-attractor-deposit-is-normalized-by-particle-count.md)'s
Alternative C: all six shipped presets want the same answer, so the default *is* the decision, and
a key would let a preset re-arm an artifact that no capture in this project can currently see.
Worth revisiting only if a preset turns up that genuinely wants the wipe.

## Notes

Raised by the user on 2026-08-03 against `attractor_ink` at `Rich`
([backlog 0050](../design-backlog.md)), and verified against
`core/src/render/scenes/particles/mod.rs:185-193` and `:1143-1155`. It sharpens
[backlog 0031](../design-backlog.md), which recorded the transient's severity at `Rich` without the
reason it is a rectangle.
