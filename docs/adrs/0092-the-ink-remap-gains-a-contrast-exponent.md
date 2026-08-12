# ADR-0092 — The ink remap gains a contrast exponent

> **Status:** accepted 2026-08-11 (user approval at the Plan 0075 handoff), with a dated
> [Outcome](#outcome--2026-08-12-at-plan-0078s-close) added at Plan 0078's close — nothing here
> was falsified; two implementation decisions the decision did not specify are recorded there.
> **Date:** 2026-08-11
> **Related plan(s):** [0078](../plans/done/0078-the-ink-learns-to-bite.md)
> **Resolves:** [design-backlog 0084](../design-backlog.md#0084--the-ink-stage-has-no-contrast-lever-and-three-worlds-in-two-cohorts-paid-for-it)
> **Supplements:** [ADR-0028](0028-final-stage-ink-tone-remap.md) (the ink stage)

## Context

The terminal ink stage (`core/src/render/ink.rs`, ADR-0028) remaps the composited frame to a
paper→ink duotone keyed on luminance, with a **fixed response**: the mapping between "how
bright" and "how much ink" is the identity, and nothing on the preset surface reshapes it.

Plan 0075's cohorts hit that absence **three times across two cohorts** (backlog 0084) —
each time wanting the same thing: a duotone whose dark pole bites harder *without moving the
paper*. The surface offers exactly two workarounds and both pay:

- **Author the duotone into `[palette]`** — the Etching world did this. It works, and it
  spends the palette on the remap's job, so the palette can no longer do its own (and the
  two cannot be composed).
- **Juggle `brightness` / `fade`** — raises contrast by trading away structure, which is
  the trade the ink look exists to avoid.

Three demonstrated wants is this project's promotion bar cleared with room; the question an
ADR owes is only *where the lever lives and what shape it has*.

## Decision

The ink remap's luminance key gains a **response exponent**: one new bindable engine-stage
param (working name `ink_gamma`; the final name and its `presets/README.md` row are fixed at
implementation) applied to the key before the paper→ink mix — `mix(paper, ink, luma^g)`
rather than `mix(paper, ink, luma)`. The endpoints are invariant by construction — `0^g = 0`
and `1^g = 1` for any positive `g` — so the paper never moves and full ink never moves;
only the response between them reshapes. That invariance *is* the requested property.
Default `1.0` is the exact identity, so nothing shipped changes.

## Consequences

### Positive

- The three-times-demonstrated want is one param: `g > 1` thins the mids toward paper so
  only the strong strokes carry full ink (the "bites harder" reading); `g < 1` inks the mids
  for a heavier, flatter print. Continuous, bindable, composable with everything else.
- Etching's palette is freed to do palette work; the ink worlds stop paying structure for
  contrast.
- Zero-baseline claim is structural (default is the identity), verified rather than assumed
  in the paired plan.

### Negative

- One more param on the ink roster, which ADR-0028 kept deliberately small.
- The exponent interacts with `ink_amount`'s blend and with `exposure` upstream of the ink
  stage — three levers now shape one response, and the README must say which does what or
  authors will ladder it out themselves (the 0061/0063 documentation lesson).
- An exponent is a one-parameter family; a look wanting an S-curve (toe *and* shoulder)
  still cannot have one. Accepted: nothing measured asks for it yet.

## Alternatives considered

### Alternative A — keep authoring the duotone into `[palette]`

Proven possible by Etching and rejected as the *answer*: it occupies the palette, cannot be
combined with a palette that does its own work, and each world re-derives it by hand. A
workaround that must be re-invented per preset is the pattern backlog 0060 exists to catch.

### Alternative B — a parametric contrast curve or a third (mid) stop

A three-tone remap is backlog 0069's composite question — the additive pipeline cannot hold
a dark edge, and the two-pole `mix` cannot hold three tones — and a full curve is more
surface than the demonstrated want needs. The exponent is one param, invertible, and covers
all three measured cases; the curve stays available as a future supplement if an author
hits the exponent's ceiling.

### Alternative C — do nothing (keep the brightness/fade juggle)

Measured insufficient three times; it is the defect, not a mitigation.

## Outcome — 2026-08-12, at Plan 0078's close

**Nothing above was falsified.** `ink_gamma` ships with the shape this ADR specified, and the two
claims that were arguments when this was written are now measured: the endpoints are invariant
across a ladder of exponents *and* across hostile values (0, negative, NaN, ±∞), and the default
moved zero pixels. Three things the decision did not say, recorded here because implementation had
to decide them:

- **"Default `1.0` is the exact identity" had to be *built*, not inherited.** `pow(x, 1.0)` is
  `exp2(1.0 * log2(x))` and is not bit-exact for arbitrary `x`, so the shader takes an explicit
  `g == 1.0` branch. Without it this ADR's zero-baseline sentence would have been false by a
  rounding step on every shipped ink preset. The guard that clamps `g` deliberately does not
  perturb `1.0` on the way to the uniform, so the branch stays reachable.
- **The exponent is clamped finite into `0.05 .. 20`, on the CPU side.** This ADR says the
  endpoints are invariant "for any *positive* `g`" and left the negative and zero cases unstated;
  a bound expression can sweep through both, and `pow(0, 0)` is undefined. The clamp lives in Rust
  rather than WGSL so `1.0` reaches the shader exactly and so WGSL's implementation-defined
  `clamp`-with-NaN is never reached. Both bounds are far outside anything a look wants (at
  `g = 0.05` a key of 0.5 reads 0.97; at `g = 20`, 1e-6).
- **It crossfades across a cross-preset dissolve**, CPU-side alongside `ink_amount`, rather than
  snapping. Unlike the poles it is a scalar on the key, so there is no colour-space question to
  answer and no second uniform slot needed.

**The zero-baseline claim is structural in a stronger sense than the paired plan argued.** No
golden fixture binds `ink_amount` at all, and the stage builds its resources only when active — so
no committed baseline ever constructs the ink pass, let alone keys through it. That also covers the
one thing a param grep cannot see: the `COPY_DST` usage flag added to `ink-src` for the endpoint
test (the arrangement `tonemap-src` already carried).

**The Negative about the one-parameter family stands untested.** Plan 0078's Phase 3 — the content
lane re-judging the ink worlds — is outstanding at that plan's close, so whether an author hits the
exponent's ceiling and wants a toe *and* a shoulder is still unmeasured. That is the trigger this
ADR named for revisiting Alternative B.
