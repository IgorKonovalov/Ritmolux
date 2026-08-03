# ADR-0050 — Downbeat and phrase tracking: bar-aware time variables, gated by a measured confidence with a deterministic counter fallback

> **Status:** accepted (Plan 0048, closed 2026-08-03 — see the Outcome section; the lock-rate
> question it raises is [backlog 0042](../design-backlog.md), an owed supplement)
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

## Outcome (added 2026-08-03 at Plan 0048's close)

The design shipped as written, all three pinned tests exist, and the Negative section's own
first bullet is what happened.

**The gate never opened wrong, and it also barely opened.** Plan 0048 Phase 6 logged 8.8
minutes of real music at 1 Hz — 458 rows with signal, roughly half beat-driven 4/4. No
confidently-wrong bar line was observed, so the plan's stopping condition did not fire. But
`downbeat_locked` was true in **14 of 458 rows (3.1 %)**, confidence sat at **mean 0.030,
median 0.000** against `CONFIDENCE_THRESHOLD = 0.25`, and the gate cleared in **two of eighteen**
30-second windows — peaking at 0.516, so the estimator *can* lock and simply rarely does.

**So the "never locks wrong" claim is un-falsified rather than confirmed**, and this record says
so plainly: with the gate shut 97 % of the time there was little opportunity for a mis-accent.
Layer 2 is, on this evidence, "a fancy counter in practice" — the risk this ADR named. That is
the designed safe floor working, not a defect: the worst case is exactly Alternative A, the
option the interview declined.

**What this must not be read as recommending.** Lowering `CONFIDENCE_THRESHOLD` to buy lock rate
inverts the reason this ADR exists. Whether to improve the *estimator* or re-price the *gate* is
a supplement to this ADR, not an edit to it — routed to [backlog 0042](../design-backlog.md).

**One design property earned more than it promised.** Locked and unlocked output come from one
formula differing only in whether the alignment is the estimate or `0`, so the fallback is not a
separate code path that can rot — and `bar_index` is `(beat_index - alignment) / 4`, which means
a lock can make it repeat or skip exactly one bar. That is documented at all three authoring
sites rather than papered over with a second counter (Plan 0049's Phase 5 item 4 weighed and
declined the counter: it would need history-dependent state on the determinism-sensitive path to
buy immunity from the soft failure this gate already prefers).

**Content guidance that follows:** build arcs on Layer 1 (`beat_index`, `time_since_beat`,
unconditional) and treat Layer 2 (`beat_in_bar`, `bar_index`, `bar_phase`) as decorative until
the estimator earns its gate. Plan 0048 Phase 7's retune was run under exactly that
qualification.
