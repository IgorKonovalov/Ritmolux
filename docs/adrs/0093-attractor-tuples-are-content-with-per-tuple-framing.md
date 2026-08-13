# ADR-0093 — Attractor tuples are content: a curated roster with per-tuple framing, and morph paths only where measured

> **Status:** accepted 2026-08-11 (user approval at the Plan 0075 handoff) — with an
> [Outcome](#outcome--2026-08-13-at-plan-0079s-close) added at Plan 0079's close: nothing here
> was falsified, and three things happened that this ADR does not describe
> **Date:** 2026-08-11
> **Related plan(s):** [0079](../plans/done/0079-the-attractor-learns-new-figures.md)
> **Resolves:** [design-backlog 0055](../design-backlog.md#0055--the-attractors-shape-vocabulary-is-breathe-and-bend-and-the-reference-figures-ask-for-more)
> **Supplements:** [ADR-0068](0068-the-projection-basis-is-a-per-family-property.md) (the per-family projection),
> [ADR-0066](0066-a-reseed-disturbs-the-cloud-rather-than-replacing-it.md) (reseed semantics this must not break)

## Context

The attractor's coefficients `a`/`b`/`c`/`d` are bindable, and `presets/README.md` tells
authors to move them "slowly and by a little" for a measured reason: these are chaotic maps,
so past a small step the figure **cuts** to a different attractor rather than morphing. The
shipped vocabulary is therefore "breathe and bend around one figure" — while the reference
galleries the user compares against are *collections of different figures*, each its own
widely-separated coefficient tuple (backlog 0055, raised by the user 2026-08-04).

Plan 0075 cohort 5 sharpened the entry from a wish into a measured wall (re-raise,
2026-08-11): a Lorenz at rho ≈ 100 — the torus-knot regime — was considered for the cohort
and is **unreachable**, because `AttractorFamily::projection()` and `seed_box()`
(`core/src/render/scenes/particles/family.rs`) are per-family **constants sized to the
canonical tuple**. A wild tuple renders off-centre and out of frame with no preset-side
recovery (`pan` cannot span it). So "new figures" is not a coefficient-freedom problem —
that freedom exists — it is a **framing** problem: a tuple is only reachable with its
projection and seed box carried beside it.

One coupling makes a naive fix dangerous, and it is already on record (the Plan 0062
architect note): `jitter_extent` is *derived* from `seed_box`, which is how one `reseed`
constant serves a map bounded in `[-2, 2]` and a flow spanning ±26. Framing data that does
not travel with the tuple silently breaks the reseed lever.

The user decided the shape at the 2026-08-11 interview: the roster **and** morph paths —
accepting that paths are research risk.

## Decision

Each attractor family gains a **curated tuple roster**: a table whose entries carry the
coefficient tuple *and its framing* — projection basis and seed box, with `jitter_extent`
derived per-entry exactly as it is derived per-family today, so `reseed` keeps its meaning
on every tuple for free. Entry 0 is the canonical tuple with today's constants, and a preset
that binds nothing is byte-identical to today. The preset surface selects an entry through a
quantized integer `tuple` param (CPU-side quantization, beside `kaleido_spiral`'s — an eased
fractional index must never interpolate coefficients between chaotic figures); a change
lands as a cut softened by the ADR-0066 reseed disturbance, and a cut between *presets* is
already hidden by the ADR-0024 dissolve. On top of the roster, **named morph paths** ship
between roster pairs — but only where an end-to-end rendered sweep shows the figure staying
recognisable along the walk (the IFS five-pair filmstrip precedent). A pair that fails the
sweep does not ship, and **zero shipped paths is a recorded outcome, not a failure** of the
plan; the roster alone already buys the gallery variety.

## Consequences

### Positive

- The reference-gallery variety becomes preset-reachable: pick a figure, not just breathe
  around one — including regimes that were physically unreachable (the rho ≈ 100 Lorenz).
- Framing travels with the tuple, so `reseed`, the dissolve, and the depth cues keep
  working on every roster entry without per-preset rescue.
- Where a morph path survives measurement, a preset can *travel* between figures — the
  strong form of the original ask — without the engine promising a walk that chaos forbids
  in general.

### Negative

- A curated data table is a maintenance surface, and curation is `human` work — the roster
  is only as good as the picks, and a bad tuple ships with its own framing looking
  authoritative.
- A quantized param inherits the smoothing seam (an eased `tuple` sweeps through *rosters
  entries*, not invalid math — but a fast binding still reads as a slideshow); the README
  must carry the same long-`[smoothing]` guidance `kaleido_order` has.
- The morph-path phase may ship empty after real render cost — accepted explicitly by the
  user; the sweep evidence is kept either way.
- The roster grows the attractor's already-longest param surface by at least one
  (`tuple`, plus whatever the path walk exposes).

## Alternatives considered

### Alternative A — free coefficient binding alone (the status quo)

Already exists and is the measured wall: it cuts between figures, and it cannot reach a wild
tuple's framing at all. Cohort 5 shipped around it; keeping it as the only route keeps the
gap.

### Alternative B — cross-fade two attractor instances

Two particle buffers and two compute dispatches on the heaviest scene in the library, to buy
a figure-to-figure blend that the ADR-0024 cross-preset dissolve already provides at zero
new engine cost. Backlog 0055's own analysis; still decisive.

### Alternative C — per-frame auto-centering on the projected centroid

Would make any tuple self-framing, but the centroid is a per-frame readback and the frame
loop must not read back (the same reason backlog 0061's option 2 stalled) — and it fights
deliberate off-centre framing besides. A static per-tuple table is the readback-free form of
the same idea.

## Notes

The curation inputs are the reference galleries backlog 0055 cites (the `de jong strange
attractor` image sweep) plus the cohort-5 want (torus-knot Lorenz). Candidate-tuple contact
sheets are the concrete-examples workflow the user prefers; the paired plan phases it that
way.

## Outcome — 2026-08-13, at [Plan 0079](../plans/done/0079-the-attractor-learns-new-figures.md)'s close

**Nothing above is falsified.** The roster shipped with per-tuple framing, entry 0 is the
canonical constants byte-for-byte, `tuple` quantizes CPU-side, and the Plan 0062 `jitter_extent`
coupling survives by construction. Three things happened that this ADR does not describe, all
recorded here rather than by editing the body.

**1. The accepted research risk did not materialise — four morph paths ship.** The Consequences
name "the morph-path phase may ship empty after real render cost" as the price the user accepted.
Twenty candidate pairs were swept; four were refused *by measurement* before any eye reached them
(a tuple partway between two others can collapse to a fixed point, whose extent is zero and which
therefore has no scale to render at — `TupleWalk::build` returns `None`, and all four are on the
discrete maps); four were judged **in motion in the running app** and kept, shipping as presets
(`thomas` 5→8, `lorenz` 0→1, `lorenz` 0→4, `de_jong` 1→3); twelve strips are rendered and
deliberately recorded as **unjudged** rather than waved through. The finding worth carrying: where
a roster walks a *single* coefficient — Thomas's `a`, Lorenz's `rho` — neighbouring entries are
neighbouring *figures* and the walk holds. The discrete maps are the harder case in both
directions.

**2. Framing alone was not enough: a measured entry seeds from an on-attractor bank.** This ADR
argues framing and stops there, and framing alone leaves a multi-second transient that overruns
the frame — the rho ≈ 100 entry wanders out to **2.2x its own extent** for its first several
seconds, because a uniform fill of a chaotic figure's bounding box puts most of its particles
*off* the attractor. The fix is ADR-0087's IFS argument extended to a figure with no closed-form
fixed points: the measurement **visits** the attractor thousands of times while framing it, so it
banks 4096 of the points it saw and a measured entry starts on itself. Entry 0 banks nothing by
design, which is what keeps every golden baseline blessed against its box fill. The cost is 48 KB
per measured entry, held for the life of the preset.

**3. The walk drives the existing `morph` param rather than a new one.** This ADR says "named
morph paths" without saying what moves along them. The implementation gave `morph` — until now
IFS-only in effect — a second meaning on the four map families, gated on a structural
`[particles] tuple_from`/`tuple_to` pair, with a tuple path on an IFS a **load error** rather than
a silent no-op. That keeps the param surface at the "+1" this ADR's last Negative budgeted for
(`tuple` alone) instead of "+2", and `without_a_path_morph_leaves_a_map_family_alone` pins the
inertness that makes it safe. Both ends are structural because the walk's framing is measured
across it at load — a near end that moved per frame would re-measure inside the frame loop.

**Two costs to state plainly.** The curation kept **all 50** candidates (*"honestly I love them
all"*), so the maintenance surface is 51 tuples rather than the short roster this ADR imagined,
and a preset load now measures every entry of its family's roster (~3.7 ms per entry in a debug
build, an order less in release, once per preset switch and never per frame). And **an entry's
index is a preset-visible name** — shipped presets step and pin them — so the table is
append-only in practice; reordering renames figures out from under content.
