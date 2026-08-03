# ADR-0063 — A preset addresses the spectrum by frequency, not by array position

**Status:** proposed
**Date:** 2026-08-03
**Related:** [ADR-0036](0036-preset-reachable-spectrum.md) (`bin(x)`, and the deferred
`bin_range`), [ADR-0049](0049-analysis-v2-dual-resolution-axis-normalized-bands.md) (the axis
rebuild that moved every probe), [design-backlog 0044](../design-backlog.md).

## Context

`bin(x)` addresses the analysis spectrum by **normalized array position**. `x` says *31 % of the
way along the band array*, and what frequency that is depends entirely on how the engine currently
lays out its bands.

Plan 0048 Phase 1 relaid them. The old axis floored every band at one FFT bin, which bound the
bottom half of the array linear; the dual-resolution window made the curve genuinely logarithmic
end to end. Every probe below the ~246 Hz crossover therefore moved by roughly an octave and a
half — silently, because a re-pointed probe is still a valid expression that still returns a number
and still renders.

The concrete damage, found only because Phase 7 happened to be reading those files: `fragment_aurora`
sets its colour from the contrast between an air probe and a low-mid probe, chosen explicitly so
that **loudness** cannot move the curtain. Its low probe was `bin(0.14)` for ~246 Hz. On the
rebuilt axis that position reads **~84 Hz** — a kick probe, which would have lurched the curtain
green on every bass hit, inverting the one property the preset exists to have.
`attractor_dejong`'s header names 65 Hz as the mistake an earlier revision made, and 65 Hz is
precisely what its `bin(0.10)` came to read.

Nothing failed. Nothing could:

- The behavioral gates render and diff; a probe pointing at the wrong frequency still renders.
- Reachability watches forks; `bin(0.14)` is not a fork.
- `core/src/dsp/fft.rs:736`, `band_for_freq_agrees_with_the_edge_table`, checks the lookup function
  against the edge table — but Phase 1 moved **both**, together, so it passed unchanged through the
  rebuild. It is an internal-consistency test with no external anchor: two sources that agree on
  every configuration we test, so no test can tell you which one the content depended on.
- The intent — "this probe is the 246 Hz low-mid" — existed only in a `#` comment.

There is a second, older request pointing the same way. ADR-0036 deferred `bin_range(lo, hi)`
because one call sees ~2 of 64 bands and averaging a few calls spot-samples a region rather than
integrating it; Plan 0048's followups revive it now that the axis resolves. "Give me a region" and
"give me a frequency" are the same underlying complaint: **a preset cannot say what it means about
the spectrum**, only where to poke it.

## Decision

We will extend the expression grammar so a preset names **frequencies in Hz**:

- `bin_hz(hz)` — the spectrum at a frequency, replacing position arithmetic at the call site.
- `bin_range(lo_hz, hi_hz)` — the mean across a frequency span, which is the region integrator
  ADR-0036 deferred, expressed in the same units.

`bin(x)` stays. It is correct for anything genuinely positional — sweeping a probe with `time`,
or `index`-driven per-element reads on `spectrum` — and removing it would break shipped content
for no gain.

Separately and immediately, we will **pin the axis with an external anchor**: a test asserting the
Hz that a handful of `x` positions resolve to, so a future layout change fails loudly and whoever
makes it knows a library sweep is owed. This is the cheap half and does not depend on the grammar
work.

## Consequences

**Positive**

- A preset states its intent in the units the intent is actually in, and survives any future
  re-banding by construction. The class of silent re-pointing this ADR exists for stops being
  possible.
- `bin_range` finally gives the region read that `bass`/`mid`/`treb` provide only at three fixed,
  very coarse widths — the gap ADR-0036 named and parked.
- The axis anchor test is small, independent, and closes the specific hole that let Phase 1 through.
- Preset comments stop carrying load-bearing frequency claims that nothing verifies.

**Negative**

- **Two ways to say nearly the same thing.** `bin(0.31)` and `bin_hz(247)` will coexist, and the
  docs have to be clear about which to reach for or the surface just gets wider.
- `bin_range` costs more per evaluation than `bin` — a span rather than an interpolation between
  two neighbours — on a path that runs per binding per frame. Bounded (64 bands maximum) but not
  free, and it wants measuring against NFR §3 rather than assuming.
- A frequency outside the analysed range has to resolve to *something*. `bin()` is total by
  construction (clamps, never errors, no panic path) and these must be too, which means quietly
  clamping a request for 25 kHz rather than telling the author it was nonsense.
- Existing content does not benefit until someone migrates it, and migration is a content pass with
  its own risk. This ADR does not require one.

## Alternatives considered

**Do nothing, plus a documented rule that an axis change is a preset-sweep event** — treat it the
way a C ABI change is treated: a known blast radius that the changing plan owns. Rejected as the
whole answer because the sweep is manual and the failure is invisible; ADR-0049 *did* carry a
one-time retune, and this still slipped through it, in the same plan. A rule that depends on
someone remembering a coupling that nothing surfaces is the rule that just failed. The doc note is
worth having, and it is not sufficient.

**The axis anchor test alone** — pin the Hz, skip the grammar. Genuinely tempting: it is a few
lines and it converts a silent re-point into a loud one. Rejected as sufficient because it makes
the change *noticed*, not *survivable* — the content still has to be found and hand-migrated every
time, and the intent still lives only in comments. Adopted as the immediate half of this decision
rather than as an alternative to it.

**Make `bin(x)` itself frequency-addressed** — reinterpret the existing argument as Hz. Rejected:
it breaks every shipped preset at once, silently, in exactly the manner this ADR exists to prevent,
and it removes positional addressing that `index`-driven `spectrum` bindings legitimately need.

**Per-preset declared band edges** — let a preset define its own array. Rejected as far too much
surface for the problem: it moves DSP configuration into content, multiplies what the analyzer must
support per frame, and no request has ever asked for it.
