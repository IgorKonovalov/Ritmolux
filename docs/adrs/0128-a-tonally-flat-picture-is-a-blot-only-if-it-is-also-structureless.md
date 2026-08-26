# 0128 — A tonally flat picture is a blot only if it is also structureless

> **Status:** proposed
> **Date:** 2026-08-26
> **Related plan(s):** [0116](../plans/0116-the-sanity-lens-finds-the-ground.md) Phases 8-9
> **Relates to:** [ADR-0126](0126-the-sanity-lens-measures-departure-from-the-frames-own-ground.md)
> (the decision this completes — 0126 re-bases *which pixels are the figure*, this one fixes *what
> the figure's tonal spread is allowed to mean*), [ADR-0123](0123-a-flat-graphic-scene-paints-its-own-paper-and-composites-opaque-elements-in-one-pass.md)
> (the scene family that forces it)
> **Raised from:** [Plan 0116](../plans/0116-the-sanity-lens-finds-the-ground.md) Phase 1's measured
> table

## Context

`MAX_TONAL_FLATNESS` fails any preset with more than 90 % of its **lit** pixels inside one of
sixteen luminance bands. Its docstring frames that as catching a figure "stacked past the additive
ceiling", and it earned its place: four attractor presets shipped as saturated single-tone masses
behind a gate that could only ask *is something there* and *is it more than a dot*.

ADR-0126 diagnosed the statistic's known false positive as a **ground** problem. A `fragment_field`
preset posterized to black, white and one red reads `flatness = 0.9346`, and 0126 attributed that to
its black ink being excluded as unlit, "leaving the statistic to measure the white alone". Plan 0116
Phase 1 built the harness to choose a ground estimator and **falsified that diagnosis on the way**.

The frame solves exactly. From `coverage = 0.5339` against black and `coverage = 0.4952` against the
paper, `Tiled Rosette Mono` at full excitation is 50.5 % white paper, 46.6 % black ink and 2.9 % red.
Both flatness readings follow from those shares alone: `0.5048 / 0.5339 = 0.9455` against black
(measured `0.9346`, the gap is antialiasing) and `0.4661 / 0.4952 = 0.9412` against the paper
(measured **`0.9413`**). Three independently-derived ground estimators all found the paper correctly
at `(245,245,245)`, and all three still convicted the preset — `0.9413`, `0.9419`, `0.9413`, every one
of them *worse* than the black reading they replaced.

The mechanism is symmetric and no estimator escapes it. `tonal_flatness` is the share of **lit**
pixels in one band, and "lit" means "not the ground". A duotone world has exactly two large
populations, so removing whichever one is the ground leaves the other holding essentially all of what
remains. Grounding the black leaves the white; grounding the white leaves the black. The only
reference that would pass is the 2.9 % red — grounding the *least* populous tone gives
`0.4661 / 0.9709 = 0.52` — and "ground = the rarest tone" is not a ground estimator and would re-base
the other forty presets into nonsense.

So the statistic is not measuring wrongly. It is measuring correctly and the gate is reading the
measurement wrongly: `MAX_TONAL_FLATNESS` encodes an unstated **luminous-world premise**, that a
figure without tonal spread must have been driven until it lost it. A flat-graphic composition has no
tonal spread *by construction*, and ADR-0123 puts a whole scene family in that world.

Phase 1 surfaced a second residue with the same shape. The estimator clears the `coverage = 1.0000`
degeneracy for the eight presets that have a ground and cannot clear it for the four that do not:
`Sumi`, `Whorl`, `Supernova` and `Neon Tunnel` are smooth luminous fields whose modal band holds
`7.4 %`, `5.0 %`, `0.7 %` and `0.3 %` of the frame — at or under the `6.25 %` a *uniform*
distribution would put in one of sixteen bands. Their coverage of 1.0 is honest; the scene really
does light every pixel. **Both residues are the same unanswered question** — *is this full-coverage
frame a composition or a fill?* — and it is a question about structure, which no choice of reference
tone can answer.

## Decision

We will make tonal flatness a **necessary but not sufficient** condition for conviction: a picture is
a blot when it is tonally flat **and** spatially structureless, and the gate fails it only on both.
Tonal flatness alone stops being a verdict and becomes one of two terms.

The second term is an **absolute structural statistic over the lit mask** — how much boundary the lit
set has for the area it covers, rather than how its tones are distributed. A saturated mass has a
short boundary for a large area; a tiled rosette, a stroke lattice or any composed graphic has a long
one. It is deliberately computed on the **binary** mask and not on tone, so it is orthogonal to the
statistic it qualifies rather than a restatement of it.

**This ADR does not name the statistic's threshold, and Plan 0116 Phase 8 is a measurement phase for
exactly that reason.** The precedent is one plan old: ADR-0126 named a diagnosis without measuring it
and Phase 1 falsified it. Phase 8 measures candidate structural statistics against the frozen
`Blown Out` fixture, the held `Tiled Rosette Mono`, and the whole shipped library, and Phase 9 applies
whichever separates the first two while moving no shipped verdict. **If none separates them, Phase 9
does not run** and this ADR gains a dated `Outcome` recording that the structural approach did not
survive contact — the same stop condition ADR-0126 carried, kept for the same reason.

## Consequences

### Positive

- A flat-graphic composition stops being convicted for having no tonal modelling, which is the idiom
  ADR-0123's scene family is built on and the reason `fragment_tiledmono` is held out of the shipped
  set today.
- The original true positive is preserved rather than traded away. A saturated additive mass is flat
  **and** structureless, so it fails both terms; the four attractor presets that motivated the
  ceiling would still be caught.
- The gate stops rewarding the workaround it currently creates. An ink of `#010101` passes the lit
  test and is visually indistinguishable from black, so the cheapest way past today's ceiling is to
  tickle `EPS` — which ADR-0126 already named and which this removes the incentive for.
- The same statistic is the instrument the *full-coverage* question needs. This ADR does not spend it
  there, but it stops that question being uninstrumented.

### Negative

- **A second statistic is a second thing to calibrate, and it has a resolution hazard this project
  has already been bitten by.** Design-backlog 0072 measured that a hairline over a 46-fold ornament
  aliases to almost nothing at the suite's 96x96 capture, which is what made `coverage` a halo-meter
  for thin strokes. A boundary-length measure is a *ratio* and so far less exposed than an areal one,
  but Phase 8 must measure it on thin-stroke content rather than assume the difference.
- **Two terms mean a conviction needs both, so the gate is strictly weaker than it was.** A picture
  that is tonally flat and structurally busy now passes. That is the intended change and it is also a
  real loss of sensitivity: a defect with that signature would ship.
- The failure message gets harder to write. "This is flat" was actionable; "this is flat and has no
  structure" has to tell an author which of the two to fix.
- `MAX_TONAL_FLATNESS`'s own derivation — half a distribution the file prints — is unchanged, but its
  *meaning* narrows, and the long doc comment arguing it from the library's distribution has to say
  so or it becomes the most confidently wrong paragraph in the file.

### Neutral

- The four luminous fields still reading `coverage` near 1.0 remain unmeasured on the
  composition-or-fill axis. Instrumented, not answered.

## Alternatives considered

### Alternative A — raise `MAX_TONAL_FLATNESS` above the graphic idiom

`Tiled Rosette Mono` reads `0.9413`; the `Blown Out` fixture reads `0.9815`. A ceiling at `0.95`
would pass one and fail the other today. Rejected on two counts. The margin is `0.04` on a statistic
whose shipped distribution already reaches `0.8839` (`Rose Web`, a legitimate figure), so the gate
would sit inside its own noise between a real preset and a purpose-built defect. And it does not
generalize: a *denser* duotone — more ink, less paper — moves toward `1.0` from below and meets the
new ceiling exactly as it met the old one. It reprices the wall rather than removing it, which is the
same objection this project raised against a per-family thin-stroke coverage floor.

### Alternative B — measure tonal flatness over all pixels rather than lit ones

ADR-0126 rejected this and the rejection stands unchanged: an additive blot covering 30 % of a black
frame reads `0.70` and passes, because the background becomes the dominant population. Re-basing the
ground does not rescue it — the arithmetic is the same whichever tone the 70 % is. It fixes the false
positive by discarding the true positive.

### Alternative C — a test-side roster of graphic presets

`KNOWN_FLAT` by another name, rejected in ADR-0126 for the reason that list is documented with:
*"if one ever goes over, that is a defect to route, not an entry to re-add"*. A second list with the
same shape and a friendlier name defeats the same rule. It is also unfalsifiable in the direction that
matters — a genuinely broken graphic preset gets the exemption its family earned.

### Alternative D — add hue spread as the second term

Attractive because a duotone is achromatic and a blot is saturated, so the two look separable on
colour. Rejected: `Tiled Rosette Mono` is black, white and red, and both of its large populations are
achromatic, so a hue-spread term reads *zero* for the preset it is meant to rescue. It answers a
question about palette where the gate is asking one about composition.

## Notes

Plan 0116 Phase 1's harness (`each_candidate_ground_is_tabled_against_the_library`, `#[ignore]`d in
`core/tests/sanity.rs`) prints every number cited above and is re-runnable; the shares in Context are
derived from its `LOUD` block.
