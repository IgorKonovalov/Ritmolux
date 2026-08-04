# ADR-0082 — The downbeat gate holds, and the estimator is diagnosed before it is tuned

> **Status:** proposed
> **Date:** 2026-08-04
> **Related plan(s):** [0068](../plans/0068-why-the-downbeat-rarely-locks.md)
> **Supplements:** [ADR-0050](0050-downbeat-and-phrase-tracking-with-confidence-fallback.md)

## Context

[ADR-0050](0050-downbeat-and-phrase-tracking-with-confidence-fallback.md) built downbeat tracking as
two layers. Layer 1 — `beat_index`, `time_since_beat` — is unconditional. Layer 2 — `beat_in_bar`,
`bar_index`, `bar_phase` — rides a confidence-gated estimator: accumulate a bass-weighted accent per
beat, fold eight bars of history over the four candidate 4/4 alignments, publish the strongest only
if the effect size clears `CONFIDENCE_THRESHOLD = 0.25` (`core/src/dsp/downbeat.rs:55`). Below the
gate it falls back to counters, and the fallback shares the publishing formula rather than being a
parallel path, so "worst case it behaves exactly like the counters" is structural.

That design was made because **a confidently wrong beat 1 is worse than no beat 1**: a preset
accenting the downbeat on beat 3 reads as broken in a way a plain counter never does.

[Plan 0048](../plans/done/0048-analysis-v2-and-the-retune.md) Phase 6 measured what it actually does
on real music — 8.8 minutes through the live app on the `v0.28.1` release build, 517 log rows at
1 Hz, 458 with signal, roughly half beat-driven 4/4:

- `downbeat_locked` true in **14 of 458 audible rows — 3.1 %**, roughly 6 % over the beat-driven half.
- `downbeat_confidence` **mean 0.030, median 0.000** against the 0.25 gate.
- It cleared the gate in **two of eighteen** 30-second windows and peaked at **0.516** — twice the
  gate.

So the estimator is capable of locking and rarely does. Every preset binding layer 2 today is
binding the fallback, and the shortfall is a *shortfall*, not a defect: ADR-0050's safe floor is
working exactly as specified.

**The measurement also did not test what it is most often quoted as testing.** No confidently-wrong
bar line was observed — but with the gate shut 97 % of the time there was almost no opportunity for
one. The mis-accent question is untested, not answered.

There is a tempting move here and it is the wrong one. Recorded as
[design-backlog 0042](../design-backlog.md#0042--the-downbeat-estimator-locks-on-3--of-audible-time-so-the-gated-bar-variables-are-almost-always-fallback).

## Decision

We will **not move `CONFIDENCE_THRESHOLD` to buy lock rate**, and we will **diagnose which term is
responsible for the shortfall before changing any of them**. Three candidates are live and nobody
has distinguished them: the accent feature (is a bass-weighted per-beat accent the right signal?),
the 4/4 fold (is the hypothesis or its history window wrong?), and the confidence measure itself (is
it under-reporting a correct alignment?). The next work on this surface produces a diagnosis with an
instrument behind it, not a tuning.

If the gate ever moves, it moves **after** the estimator improves, not instead of it.

## Consequences

### Positive

- **The trade ADR-0050 was written to make survives.** Buying lock rate with the gate inverts it
  precisely: it converts a known-safe fallback into an unknown rate of confidently-wrong beat 1s,
  against a mis-accent risk the measurement did not test.
- **A diagnosis is reusable and a tuning is not.** Whichever term turns out to be weak, knowing
  which one is what makes the *next* change on this surface cheap.
- **It names the untested thing.** Writing down that the mis-accent question is untested rather than
  passed is what stops the 8.8-minute measurement being quoted later as evidence of safety.

### Negative

- **Layer 2 stays decorative for now**, and the authoring docs currently offer both layers without
  distinguishing their availability, so an author can build an eight-bar arc on a variable that is a
  counter 97 % of the time. That is a live cost and the plan owes a documentation qualification
  whether or not the diagnosis succeeds.
- **Diagnosis costs a plan and may not produce a fix.** The honest outcome "the accent feature is
  adequate and real music simply has less bass-weighted downbeat structure than the model assumes"
  is a possible finding, and it would leave the shortfall standing with a better explanation.
- **The instrument has to be built.** Nothing today prints per-alignment scores; the 1 Hz log gives
  the outcome and not the terms behind it, which is exactly why three candidate causes are still
  undistinguished after a measurement that took nine minutes of real listening.

### Neutral

- Nothing about layer 1 changes. `beat_index` and `time_since_beat` are unconditional and reliable,
  and they are what a preset should be building on today regardless of how this resolves.

## Alternatives considered

### Alternative A — Lower `CONFIDENCE_THRESHOLD`

The one-line change that would raise the lock rate immediately. Rejected because it buys the
capability by spending the guarantee: ADR-0050 exists because a confidently wrong downbeat is the
failure an author cannot work around, and the measurement that motivates the change is the same one
that had no opportunity to observe that failure. Adjusting a safety gate using data collected while
the gate was closed is circular.

### Alternative B — Accept the shortfall and document it

Cheapest and honest: layer 1 is reliable, layer 2 is decorative until further notice, say so in
`presets/README.md` and move on. Rejected as the *whole* answer — but adopted as part of it, because
the documentation qualification is owed either way. What it cannot do alone is justify the cost
already paid: the estimator, its tests and its diagnostic column are built and shipping, and
retiring their value by documentation is a strange place to stop without first finding out why they
underperform.

### Alternative C — Rewrite the estimator against a stronger model

Replace the bass-weighted accent and 4/4 fold with something more capable — a full beat-tracking
model with meter hypotheses. Rejected on evidence, not on ambition: no part of the current estimator
has been shown to be the weak one, so a rewrite would be replacing three terms to fix whichever of
them is broken, at a cost this project's dependency budget and hot-path rules would both have to
be argued against.

## Notes

- The instrument to build against is the module's own structure: `downbeat.rs` already computes four
  alignment scores and a corrected effect size; what is missing is a way to *see* them over a run.
- **Do not re-measure by ear.** The 1 Hz `downbeat_locked` column is the instrument, and a targeted
  pass over known-4/4 material only would sharpen the ~6 % figure that the half-and-half split
  leaves approximate.
- 4/4 is assumed and documented; a different meter falls back to the counters rather than
  mis-accenting. That is not a candidate cause for the shortfall on the material measured, which
  was roughly half beat-driven 4/4.
