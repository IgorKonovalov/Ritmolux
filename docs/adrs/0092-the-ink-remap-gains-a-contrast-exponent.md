# ADR-0092 — The ink remap gains a contrast exponent

> **Status:** proposed
> **Date:** 2026-08-11
> **Related plan(s):** [0078](../plans/0078-the-ink-learns-to-bite.md)
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
