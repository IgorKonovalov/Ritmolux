# ADR-0050 — Downbeat and phrase tracking: bar-aware time variables, gated by a measured confidence with a deterministic counter fallback

> **Status:** proposed
> **Date:** 2026-07-30
> **Related plan(s):** 0048-analysis-v2-and-the-retune (R5)
> **Supplements:** the beat/tempo tracker (Plan 0001 lineage), ADR-0020 (`tempo`), ADR-0036 (the analysis surface presets reach)

## Context

No preset can build an 8-bar arc, land a drop, or do anything "every 4th beat": the grammar's
only musical time is `beat` (a one-hop gate), `bar` (despite the name, *beat* phase), and
`tempo`. The visual-richness review names phrase time as half of "alive"; the 2026-07-30
interview chose the full form — real downbeat tracking — over the recommended counters-only
option and over graduating `novelty`. The user accepted the risk of research-grade DSP.

The honest risk, stated up front: downbeat estimation on real music has genuine failure
modes (syncopation, half-time feel, non-4/4), and **a wrong downbeat is worse than none** —
a preset accenting beat 1 on beat 3 reads as broken in a way a plain counter never does.
The design's job is to keep the aggressive capability while making its failure mode the
conservative behavior.

## Decision

We will add the time vocabulary in two layers. **Layer 1, unconditional:** `beat_index` (a
monotone beat counter from the existing tracker) and `time_since_beat` (seconds, resetting
each beat) — deterministic, cheap, and useful alone. **Layer 2, gated:** a downbeat
estimator that folds the onset/accent pattern over the four candidate beat-1 alignments of a
4/4 hypothesis (bass-weighted accents, a few seconds of evidence) and yields `beat_in_bar`
(0–3), `bar_index`, and `bar_phase` (0..1 across the bar) — **published only while its
confidence clears a measured threshold**. Below confidence, the same three variables fall
back to `beat_index`-derived values (`mod 4`, phase from the counter): still periodic,
still useful, never *wrong about the music*, and deterministic. The fallback is the default
state (cold start, ambiguous material, non-4/4), and switching alignment is hysteretic so
the bar line does not hop frame to frame. 4/4 is assumed and documented; `bar` keeps its
shipped beat-phase meaning (too widely bound to rename), with the docs stating the misnomer
beside the new true `bar_phase`.

## Consequences

### Positive
- "Every 4th beat", bar-phase palette sweeps, and 8-bar arcs (`mod(bar_index, 8)`) become
  one-line bindings; combined with ADR-0049's normalized bands, "the drop lands" is
  authorable without hand-tuned magic.
- The failure mode is graceful by construction: worst case, the new variables behave exactly
  like the counters the recommended option would have shipped.

### Negative
- Real DSP research risk: the estimator may clear its gate rarely on real material, making
  Layer 2 a fancy counter in practice. The plan's `human` listening phase measures this
  honestly; if lock rates are poor, that finding is recorded, not massaged.
- Hot-path cost of accent folding (small — it reuses the existing onset/flux stream — but
  measured, not assumed).
- A confidence gate plus hysteresis is more analyzer state on the determinism-sensitive
  path; all of it is a pure function of input history, tested with synthesized accent
  patterns.

## Alternatives considered

### Alternative A — counters + `time_since_beat` only (the recommendation)
Deterministic and honest, but "every 4th beat" is phase-anchored to nothing — the accent
lands wherever the counter happened to start. Rejected by the user in favor of real bar
awareness; survives inside the decision as the fallback layer, which is why the risk is
acceptable at all.

### Alternative B — graduate `novelty` into a section signal
Attacks "react to the buildup" rather than bar time; a different capability, not a cheaper
version of this one. Deferred, not refused — it stacks cleanly on top later.

### Alternative C — publish the downbeat ungated
Simpler, and wrong: a confidently wrong beat 1 is the one failure an author cannot work
around. The gate is the design.

## Notes

Test strategy pinned now: synthesized click patterns with an accent every 4th beat must
lock to the accented alignment (all four rotations); an unaccented pattern must stay in
fallback; a mid-stream alignment flip must take several bars (hysteresis), not one frame.
