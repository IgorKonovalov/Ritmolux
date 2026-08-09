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

## Outcome — 2026-08-09, after Plan 0068 Phases 1-3

**The named cause is the accent feature, and specifically its bass weighting. The confidence
measure and the fold are both exonerated as the primary term.** The decision above held: nothing
was tuned, `CONFIDENCE_THRESHOLD` is still `0.25`, and the three suspect terms are unchanged.

### The measured lock rate on known-4/4 material

The Notes above asked for a targeted pass to sharpen the approximate ~6 %. It sharpens to **6.0 %**
— 352 locked rows in 5900 audible ones, 98 minutes through the live app on the `v0.48.0` build,
split by genre:

| material | audible rows | lock rate | confidence mean | median | peak | 30 s windows cleared |
|---|---|---|---|---|---|---|
| four-on-the-floor techno | 5173 (86.2 min) | **6.79 %** | 0.0587 | 0.0000 | 0.9494 | 50 of 176 (28 %) |
| backbeat rock/pop | 727 (12.1 min) | **0.14 %** | 0.0249 | 0.0000 | 0.2664 | 1 of 25 (4 %) |

Restricting to unambiguous 4/4 does **not** rescue the rate. The audible-row filter is not
load-bearing: the rate is identical at every floor from `0.00` to `0.10`, so Plan 0048's undefined
"with signal" cut never mattered.

### Why the accent feature, and why backbeat material is *worse*

`BASS_WEIGHT = 0.7` — an accent is 70 % bass band, on the reasoning recorded at
`core/src/dsp/downbeat.rs:65-71` that "the kick is the downbeat cue in most material this targets."
**That premise is false for both genres measured, in two different ways:**

- **Four-on-the-floor**: the kick is on *every* beat, at equal weight. The bass-weighted accent is
  therefore near-uniform across all four candidate alignments, the fold is flat, and the effect size
  is near zero. The kick is a *beat* cue here, not a downbeat cue.
- **Backbeat rock/pop**: the kick is on 1 and 3, the snare on 2 and 4. The bass-weighted accent
  carries a **two**-beat periodicity, so alignments 0 and 2 score alike and the fold is
  systematically ambiguous rather than merely flat. This is why the genre with the *most* obvious
  loudness accent scores **48× worse** than the one with none — and it is the finding that falsifies
  the intuition that a clearer backbeat would help.

In both cases the downbeat in real music is marked spectrally, harmonically and structurally, not by
bass energy on beat 1.

### Where this sits on Plan 0068 Phase 2's ladder

Real material sits on the **contrast** axis at its extreme, not on the dropout axis the synthetic
ladder predicted:

| | confidence | lock rate |
|---|---|---|
| ladder, contrast rung 1.00 (all contrast lost) | 0.032 | 0 % |
| backbeat rock/pop | 0.0249 | 0.14 % |
| four-on-the-floor techno | 0.0587 | 6.8 % |

Techno scoring slightly above the rung fits its **bimodal** shape — median exactly `0.0000` with a
`0.9494` peak. The estimator sees nothing most of the time and locks hard occasionally, which is
what breakdowns and fills look like: the passages that genuinely do carry a four-beat accent
structure. That bimodality is itself evidence for this diagnosis and against a measurement defect —
a broken confidence measure would depress the peaks too.

### What this does *not* overturn

Plan 0068 Phase 2's finding stands on its own terms: η² is a between/within variance ratio, so it is
scale-free (an accent 1 % louder scores like one 100 % louder) and it penalises intermittency far
harder than weak contrast; the null correction owns about a fifth of the dropout axis's shortfall.
**That sensitivity is real but secondary** — it never gets the chance to bind, because the input
carries no beat-position contrast for it to under-report. Fixing the confidence measure alone would
not move the field rate.

### Limits of this diagnosis, stated so it is not over-read

- The 1 Hz log records **band levels, not per-beat accents**. The identification rests on the
  numeric match to contrast rung 1.00 across two genres plus the construction argument above, not on
  a direct reading of the four alignment scores on real audio. `DownbeatTracker::terms()` (Phase 1)
  is the instrument that *could* settle it, but it is test-only and not wired to the diagnostics log.
- Bands were healthy and unsaturated in both runs (bass clipped in 3.0 % of techno rows and 5.2 % of
  rock rows, median 0.447 and 0.763), so this is **not** an input-scaling or normalisation failure.
- Two genres are not all of 4/4. Material whose downbeat *is* bass-marked — much hip-hop, some
  orchestral — was not measured and may well lock; the 6.8 % techno figure shows the machinery works
  when the cue is present.
- The mis-accent question ADR-0050 exists to guard is **still untested**. With the gate shut ~94 % of
  the time there was again little opportunity to observe a confidently-wrong bar line.

### What the repair plan inherits

The route is a **downbeat cue that is not bass energy** — spectral change, harmonic/chord-change
rate, or phrase-boundary novelty — evaluated against the same ladder and the same two genres. That
is a new accent feature, not a rewrite of the estimator: the fold, the hysteresis, the gate and the
fallback are all uninvolved in this failure and Alternative C's objection to replacing three terms
still applies. Alternative A stays rejected and is now *better* refuted than when it was written:
lowering the gate against material whose fold is flat (techno) or systematically ambiguous (rock)
would buy mis-accents specifically, which is the failure ADR-0050 exists to prevent.

The honest counterfactual recorded in the Negative consequences above — "the accent feature is
adequate and real music simply has less bass-weighted downbeat structure than the model assumes" —
is **half right and worth reading again**. Real music does have less bass-weighted downbeat
structure than the model assumes. But that makes the accent feature *inadequate* rather than
adequate, because it is the feature's own weighting that chose to look there.
