# ADR-0095 — The backdrop paints a curved band under the scene

> **Status:** proposed
> **Date:** 2026-08-12
> **Related plan(s):** [0081-the-sky-gets-a-galaxy](../plans/0081-the-sky-gets-a-galaxy.md)
> **Supplements:** [ADR-0094](0094-the-backdrop-paints-a-directional-ramp.md) (the ramp this sits
> beside, in the same pass), [ADR-0086](0086-the-backdrop-colours-through-the-preset-palette.md)
> (one colour language), [ADR-0090](0090-a-preset-composes-two-scene-layers.md) (the one-`[layer]`
> budget this avoids widening), [ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md)
> (the aspect rule both axes obey)

## Context

[ADR-0094](0094-the-backdrop-paints-a-directional-ramp.md) landed the dusk ground: the backdrop
sweeps a segment of the preset's palette along one axis, so a horizon fades smoothly instead of
sitting on a hard-edged slab. Judged in the running app against the user's reference photograph — a
Milky Way arc standing over a dusk horizon — the ground reads and **the galaxy is what is missing**.

The reference's band is a soft, oriented, *curved* swell of light: brighter along a diagonal arc,
falling off smoothly to either side, with the scattered bright stars sitting in front of it. Nothing
in the engine can paint it, and the reason is structural rather than a tuning gap.

**The role budget is already spent, and stretching it is not available.** This class of look needs
four things at once — ground, galactic band, bright stars, and a reactive figure — while
[ADR-0090](0090-a-preset-composes-two-scene-layers.md) caps composition at a main scene plus one
`[layer]`. Plan 0080 answered the first of the four by taking the ground out of the scene budget
entirely. The band has to come from the same place, or it displaces the stars.

**No scene can express it.** Verified against the rosters rather than assumed: neither
`swarm::PARAMS` nor `emitter::PARAMS` carries any positional or density control — a particle scene
fills its domain the way its forces put it and there is no lever that says *more here, fewer
there*. `fragment_field` draws opaquely over the backdrop, so a preset using it has no backdrop at
all. A band is not a shape a scene can be asked for.

**And the neighbouring capability ADR-0094 already named is the wrong one.** That ADR closed by
observing that "a ramp drawn *after* the scene, occluding the bottom of the figure, is a real and
different capability" and leaving it unforeclosed. That is a foreground haze. **A galaxy is the
opposite**: it is unresolved starlight *behind* the stars, so it belongs in the same pre-pass the
ground already uses, under everything. The foreground want stays open and stays separate.

## Decision

**The background pre-pass gains one soft band, drawn additively beside the ramp, in the same pass.**
Seven bindable params, every one defaulting to an exact arithmetic identity with the picture Plan
0080 shipped:

| Param | Default | What it is |
|-------|---------|------------|
| `bg_band_amount` | `0.0` | Intensity. At `0` the band term is not added at all — a `select` arm, not a multiply by zero. |
| `bg_band_angle` | `0.0` | Radians, naming the direction **across** the band — the same axis convention `bg_angle` uses, so `0` runs a band horizontally. |
| `bg_band_pos` | `0.5` | The centreline's position along that across-axis, in the same normalized `0..1` the ramp's `s` uses. |
| `bg_band_width` | `0.15` | Gaussian half-width, in those same units. The envelope reaches `1/e` exactly `bg_band_width` either side of the centre. |
| `bg_band_curve` | `0.0` | The arc. Bows the centreline by `curve * 4t(1-t)` in the along-band coordinate `t`, so it is zero at both ends, maximal in the middle, and **exactly straight at `0`**. |
| `bg_band_hue` | `0.0` | Its own coordinate in the **same** `[palette]` the ground samples. |
| `bg_band_hue_span` | `0.0` | How far that coordinate travels **along** the band, so the galactic core can brighten toward one end. |

The band is **additive over the ground**, which is what luminous unresolved starlight is, and which
is what makes `bg_band_amount = 0` an identity rather than an approximation of one. It samples the
same LUT pair through the same `palette_mix` and `saturation` the ground does, so there is one
colour language in the frame.

**The pipeline build condition widens from `bg_bright > 0` to `bg_bright > 0 || bg_band_amount > 0`.**
Today the pass skips building its gradient pipeline entirely below a visible `bg_bright`, which
would make a galaxy over a near-black sky silently render nothing. That is a one-line change and it
is load-bearing: the reference photo's sky *is* nearly black away from the horizon.

Both axes take their aspect from the **destination surface**, which the pass has received since Plan
0080. The along-band axis is new and gets its own normalizer, so
[ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md) applies to it independently.

## Consequences

**Positive.**

- The four-role look becomes authorable without touching ADR-0090's layer cap: backdrop paints
  ground *and* band, the main scene carries the stars, and the `[layer]` is still free for a
  reactive figure.
- One colour language holds. The band is a palette segment like everything else since ADR-0086, so
  an author moves a stop and the galaxy's colour moves with the rest of the frame.
- Every default is an arithmetic identity, so no shipped preset moves and no baseline is re-blessed
  — the same discipline, and the same bless-to-bless instrument, Plan 0080 used four times.
- `curve = 0` leaves the straight band available for free. Rejecting Alternative F did not cost the
  simpler shape; it only stopped it being the *only* shape.

**Negative — the price, stated plainly.**

- **No dust lanes.** The reference's dark rifts and mottled core are exactly what Alternative A
  would have bought, and it is not being bought. The band is one smooth swell and the visible
  texture comes from the stars drawn over it. If that reads as a smudge rather than a galaxy, the
  fix is Alternative A and it is a separate decision.
- **`Bg` grows 48 → 80 bytes**, which moves this pass's `min_binding_size` for the second time in
  two plans. That field is a Plan 0053 fix against a *measured* WARP mis-render, so an adapter
  comparison is owed before any baseline is blessed. Plan 0080 established that this is cheap and
  that the ADR-0058 enumeration does not move (it records *whether* a size is declared, not which).
- **Eight `bg_*` names become fifteen.** The backdrop namespace is now the largest single group of
  engine params, and an author meets all of them at once in `presets/README.md`. That is a real
  legibility cost and it is being paid deliberately, because the alternative was a scene slot.
- **The backdrop is still invisible to every gate** (ADR-0067 coverage, ADR-0091 animation). A band
  earns a preset nothing at `sanity` or `animation`, so a sky-heavy world still has to carry both
  floors on its figure. This ADR makes the backdrop more capable *and* makes that asymmetry sharper.
- **A second wide smooth gradient, in an engine that does not dither.** Plan 0080 Phase 7's banding
  verdict is **still outstanding** at the time of writing, and this decision adds another quarter-frame
  fade before that question has been answered. If the ramp bands, the band will too, and one dither
  decision covers both.

## Alternatives considered

### Alternative A — fbm noise: mottling and dust lanes

Modulate the band's envelope with fractal noise, giving the knotted core and dark rifts that make
the reference read as a galaxy rather than a glow. **Rejected on cost, by the user's call at the
interview**, and it is the alternative most likely to be revisited.

The decisive reason is that the render layer has **no general-purpose noise primitive** — a grep
finds only `hash_unit` and `hash3` inside `scenes/particles/`, both per-particle seed derivations,
neither a spatial field. So this is not a parameter, it is a new piece of shared shader machinery
with its own determinism obligation (seeded and time-parameterized, never clock-driven, per the
project's determinism rule) and its own place to live. Against that, the scattered starfield drawn
over the band already supplies texture where the eye looks for it.

Named rather than foreclosed: the band's envelope is a single multiply, so noise multiplies into it
later without redesigning anything decided here.

### Alternative B — densify the starfield inside the band

Let the scene pack more particles where the band is, so the galaxy *is* stars, as in reality.
Physically truest and it would need no painted glow at all. **Rejected as the largest of the three:**
scenes have no density-shaping concept whatsoever, so this touches the particle systems and their
spawn distributions rather than the backdrop, and it would have to be built twice (swarm and
emitter) or once in a shared home that does not exist. It also cannot serve a preset whose figure
is not a particle scene.

### Alternative C — a post-scene alpha ramp

Draw the band *over* the scene with alpha, which is the capability ADR-0094 explicitly left open.
**Rejected because it is on the wrong side of the stars.** A galaxy behind the stars is the entire
point; painting it over them dims exactly the marks that are supposed to sit in front of it. The
foreground haze that ADR-0094 named remains a real, distinct and still-unaddressed want — this
decision does not serve it and does not foreclose it.

### Alternative D — the band gets its own explicit colour params

Two colours on the band itself, independent of `[palette]`. Simplest to author for one look, and it
avoids crowding the dusk palette. **Rejected: it puts a second colour language back into the
engine**, which is precisely what ADR-0086 spent a plan removing and what ADR-0094 declined to
reintroduce. The cost of the chosen option — a palette that must now hold both the horizon ramp and
the band's colours — is an authoring constraint, not an architectural one.

### Alternative E — the band samples palette B while the ground samples palette A

Reuses the existing pair so each feature gets a whole palette, with no crowding at all. **Rejected
on a collision:** `palette_mix` already owns that pair for preset crossfade (ADR-0020, and the
dissolve machinery drives it). A band pinned to B would fight every dissolve, and a dissolve would
recolour the galaxy on its way past.

### Alternative F — a straight band, no curvature

Angle, position, width, softness — four params, simpler shader math, and honest to what a normal
field of view actually sees, since the reference's arc is substantially a panorama artifact.
**Rejected by the user at the interview**, on the grounds that the curve is the silhouette that
makes the shape read as the Milky Way rather than as a streak. The rejection cost nothing that
mattered: `bg_band_curve = 0` is exactly the straight band, and it is the default.

### Alternative G — a `gradient` or `nebula` `SystemKind`

A scene that paints the band. **Rejected for the identical reason ADR-0094 rejected it for the
ground**, and the reason is stronger here rather than weaker: a new system would occupy the main
slot or the one `[layer]`, and with four roles wanted and two slots available that leaves two roles
homeless instead of one.

## Notes

- The reference is the user's (a Milky Way arc over a dusk horizon). The look that prompted it is
  the dusk ground from [Plan 0080](../plans/done/0080-the-sky-gets-a-horizon.md), judged live in the
  app at `v0.54.0`.
- The shipped world itself is content-lane work through the
  [ADR-0081](0081-the-content-lane-lands-presets-and-architect-curates-the-set.md) /
  [Plan 0067](../plans/done/0067-the-curation-route.md) route, and it groups with **Plan 0077
  Phase 5** (Perseids' quiet sky) and **Plan 0080 Phase 7** — three standing content items on one
  family of looks, and one pass.
- The star field's own "not all at once" shimmer is already shipped: Plan 0077's `twinkle` gives
  every mark a seeded rate *and* phase. Binding `twinkle` depth to a band makes the whole sky
  sparkle harder with the music while no two stars ever blink together. **Per-mark audio gating —
  specific stars latching on a beat — does not exist**, and is not decided here.
