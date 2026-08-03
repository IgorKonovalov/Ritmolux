# ADR-0063 — A preset addresses the spectrum by frequency, not by array position

**Status:** **accepted** — but only its *immediate half* is built.
[Plan 0056](../plans/done/0056-clamp-occupancy-and-the-axis-anchor.md) Phase 4 landed the external
axis anchor (closed 2026-08-03); **`bin_hz()` / `bin_range()` are not implemented** and remain a
followup plan with no number yet. **Carries an Outcome section**, including the two shipped probes
the anchor confirmed damaged.
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

## Outcome (Plan 0056 Phase 4, 2026-08-03) — the anchor only

**Half of this ADR is built.** The immediate half — the external axis anchor — landed as
`bin_positions_resolve_to_the_frequencies_the_presets_were_written_against` in `core/src/dsp/fft.rs`.
The grammar half, `bin_hz()` and `bin_range()`, is **not implemented** and is a followup plan.

The anchor is eight literals, measured on 2026-08-03 and written down rather than computed from the
layout function — which is the whole point, since a test that re-derives from the thing that moved
cannot notice that it moved. Its helper reads `edges_hz` deliberately: the helper *must* move with
the layout, and it is the right-hand column that must not.

```text
bin(0.00)     36.7 Hz      bin(0.31)    246.9 Hz
bin(0.10)     67.9 Hz      bin(0.50)    793.7 Hz
bin(0.14)     86.9 Hz      bin(0.84)   6413.2 Hz
bin(0.20)    125.6 Hz      bin(1.00)  17143.2 Hz
```

**Two of those corroborate this ADR's own damage claim to within its rounding.**
`attractor_dejong`'s `bin(0.10)` now reads **67.9 Hz** — the ~65 Hz its own header names as the
mistake an earlier revision made. `fragment_aurora`'s `bin(0.14)`, chosen for the ~246 Hz low-mid
precisely so loudness could not move the curtain, now reads **86.9 Hz**, a kick probe. The position
that actually reads that low-mid today is `bin(0.31)`.

The test's comment states what a failure means, because a failure there is not a bug: the axis was
relaid, and the obligation it creates is a **content sweep**. Every `bin()` in `presets/` has to be
re-checked against the frequency its author's comment named, and the literals are updated *after*
that sweep, not instead of it.

**The two shipped probes above are still mis-pointed.** The anchor makes the next re-band noticeable;
it does not repair the damage the last one did. That repair is content work and is unclaimed.
