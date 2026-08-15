# ADR-0097 — the downbeat cue is chosen against per-beat evidence, not against the ladder argument

> **Status:** accepted (2026-08-13, user approval)
> **Date:** 2026-08-13
> **Related plan(s):** [0086 — the downbeat finds a cue that is not the kick](../plans/done/0086-the-downbeat-finds-a-cue-that-is-not-the-kick.md)
> **Supplements:** [ADR-0050](0050-downbeat-and-phrase-tracking-with-confidence-fallback.md),
> [ADR-0082](0082-the-downbeat-gate-holds-and-the-estimator-is-diagnosed-first.md)

## Context

[ADR-0082](0082-the-downbeat-gate-holds-and-the-estimator-is-diagnosed-first.md) and
[Plan 0068](../plans/done/0068-why-the-downbeat-rarely-locks.md) established the shortfall and named
a cause. Over 98 minutes of unambiguous 4/4 on `v0.48.0`, 5900 audible rows, the downbeat estimator
published on **6.0 %** of them — and split by genre that is **6.79 %** on four-on-the-floor techno
against **0.14 %** on backbeat rock/pop, a single locked row in 727. The named cause is the accent
feature: `BASS_WEIGHT = 0.7` (`core/src/dsp/downbeat.rs:71`) makes the accent chiefly a kick
detector, and a kick marks every beat in four-on-the-floor and the half-bar in a backbeat, so it
hardly ever marks the *bar*.

**That cause was inferred, and ADR-0082 says so in its own `Outcome`.** The instrument was the 1 Hz
diagnostic log, which records **band levels, not per-beat accents**. So what exists is a ladder
match plus a construction argument — the genre split is exactly what a bass-dominant accent
predicts — and *not* a measurement of the four alignment scores on real audio. The decomposition
that would settle it already exists in the code as `DownbeatTerms` (`downbeat.rs:110`), built by
Plan 0068 for precisely this purpose, and it has never been run against real backbeat material.

This matters because the shortlist is not one candidate. A bass-dominant accent is consistent with
at least three different failures, and they want different repairs:

- **The accent is too narrow.** A backbeat's information lives in the snare on 2 and 4; the accent
  never sees it. Repair: a second accent band.
- **The accent is fine and the pattern is ambiguous.** A kick on 1 and 3 is 2-periodic, so
  alignments 0 and 2 score alike, and the fold is choosing between two equally good answers rather
  than failing to find one. Repair: a cue that breaks the two-fold degeneracy — harmonic change,
  not percussive weight.
- **The evidence window is wrong.** `ACCENT_HISTORY = 32` beats is eight bars; on material whose
  bar-scale structure turns over slower than that, eight observations per alignment may simply be
  thin. Repair: a longer history, which is a constant rather than a feature.

`effect_raw`, `null_share` and the four `scores` separate these three in one reading. Nothing has
read them.

The cost of guessing wrong is not a wasted plan — it is a *plausible* repair that raises the lock
rate on the material it was designed against and leaves the real cause standing, which is the
failure mode this project has now refused twice: once when ADR-0082 declined to move
`CONFIDENCE_THRESHOLD` using data collected while the gate was shut, and once when
[Plan 0067](../plans/done/0067-the-curation-route.md) Phase 1d measured a resolution ladder flat
rather than raising a constant on the strength of an argument.

## Decision

We will choose the replacement downbeat cue **against a per-beat accent decomposition captured on
real backbeat material**, not against the genre-split argument that named it. The plan captures
`DownbeatTerms` per beat on known-4/4 backbeat audio and on a four-on-the-floor control, publishes
the four alignment scores, `effect_raw` and `null_share` side by side, and only then builds the
cue — selected at a `human` gate from a shortlist ranked against that data.

A candidate refused by the measurement is a **recorded outcome, not a failed phase**, in the shape
[Plan 0079](../plans/done/0079-the-attractor-learns-new-figures.md) used for its morph paths: four
of twenty pairs were refused before any eye reached them, and that is written down as a result.

`CONFIDENCE_THRESHOLD` does not move in this plan, for the reason ADR-0082 already gave and which
this decision does not reopen: buying lock rate with the safety gate inverts the trade ADR-0050 was
written to make. If the gate moves at all it moves *after* the estimator improves.

## Consequences

### Positive

- **The three stories get told apart.** `scores` distinguishes "no alignment wins" from "two
  alignments tie"; `effect_raw` against `null_share` distinguishes "the accent carries no structure"
  from "the correction is eating a real effect". One reading, three answers, and the instrument is
  already written.
- **The shortlist is ranked before anything is built.** A second accent band and a harmonic-change
  cue are different sizes of work — the first is a weight and a band sum, the second is new DSP with
  its own determinism and no-alloc story. Knowing which one the data asks for is worth a phase.
- **It cannot repeat ADR-0082's own stated limit.** That ADR's honest caveat is that it never
  measured per-beat accents. This plan's first phase is that measurement, so the successor rests on
  evidence of the kind the predecessor said it lacked.
- **The measurement outlives the cue.** A per-beat decomposition capture is reusable by any future
  estimator change, and it is the missing half of Plan 0068's instrument — `terms()` exists and
  nothing calls it outside tests.

### Negative

- **It costs a phase and a `human` gate before any capability lands.** A plan that went straight at
  the second accent band could plausibly ship a working improvement sooner. If the measurement
  simply confirms the inferred cause, that phase bought a confirmation rather than a redirection —
  and that is a real price, paid on purpose.
- **The measurement needs real audio the repo cannot hold.** Synthetic patterns are what
  `downbeat.rs`'s own tests already use, and they are exactly what cannot settle this: a synthesized
  backbeat is a hypothesis about backbeats. So the capture runs through the live app on the user's
  own material, which makes it a `human` phase and not reproducible in CI.
- **A shortlist chosen at a gate is a decision the plan does not fully specify in advance.** The
  phases after the gate are written against a cue that is named but not yet picked, so their
  done-whens are stated as properties rather than as edits to particular lines.
- **Nothing improves for a preset author until the whole plan lands.** `beat_in_bar`, `bar_index`
  and `bar_phase` stay ~94 % counter-derived throughout, exactly as
  [`presets/README.md`](../../presets/README.md) now states.

### Neutral

- The measurement phase adds no code to the analysis path. `DownbeatTerms::terms()` is `&self`,
  allocation-free and clock-free by construction (Plan 0068), so reading it changes nothing about
  what the estimator does — the property that made it safe to ship as a diagnostic is the same one
  that makes it safe to log per beat.

## Alternatives considered

### Alternative A — build the second accent band now

Go straight at the named cause: make the per-beat accent a two-band pattern (bass plus a mid-band
flux term) instead of one bass-weighted scalar, so a backbeat's snare on 2 and 4 gives the fold
something to discriminate on. **Rejected because it commits to one of three failures that fit the
same evidence.** If the true defect is the 2-periodic degeneracy rather than the accent's
narrowness, a second band adds a *second* 2-periodic signal — snare on 2 and 4 is the same
ambiguity phase-shifted — and the lock rate would move little for a change that reads convincing.
The measurement distinguishes those cases in one run and costs a phase. Note this alternative is
not discarded: it is the leading shortlist entry going into the gate.

### Alternative B — the harmonic-change cue

Add a spectral-novelty or chord-change detector at bar scale — the standard downbeat cue in the
literature, and independent of the drum pattern entirely, which is exactly what breaks the
degeneracy. **Rejected as the opening move because of its size**, not its merit: it is genuinely new
DSP on the sacred path, needing its own determinism story, its own no-alloc proof, its own history
buffers, and its own place in the NFR §6 window budget. Committing to it before knowing whether a
band weight would have done is the expensive direction of the same guess. It stays on the shortlist,
and the measurement is what would justify its cost.

### Alternative C — lengthen `ACCENT_HISTORY` and re-measure

The cheapest possible change: more beats of evidence per alignment. **Rejected as a standalone
answer**, because ADR-0082's diagnosis exonerated the fold and the confidence measure but said
nothing about the window, and lengthening it trades responsiveness for evidence in a way nobody has
priced. It is a constant, so it is a free axis to *include in the measurement* — which is what this
decision does with it — rather than a repair to ship on its own.

### Alternative D — accept it and document it

The cheapest honest option, and [backlog 0042](../design-backlog.md) named it. **Already taken, and
it is not enough.** `presets/README.md` and `docs/presets.md` both now state the measured rate and
mark layer 2 decorative. That closed the authoring-guidance half and left the capability half
exactly where it was: the bar variables ADR-0050 designed are, in practice, counters.

## Notes

**The mis-accent question ADR-0050 guards is still untested**, and this plan does not close it
either. Both measurement passes ran with the gate shut ~94 % of the time, so there was little
opportunity to observe a confidently-wrong beat 1. Any cue that raises the lock rate materially
*creates* the first real opportunity to test it — which is a reason to watch for it at the gate, and
a reason the threshold stays where it is while the estimator changes underneath it.

**Do not re-measure by ear.** The 1 Hz `downbeat_locked` column is the outcome instrument and
`DownbeatTerms` is the decomposition; both are in the repo, and an impression from listening cannot
be compared against either.

## Outcome (2026-08-15)

**The decision worked, and it cost this ADR its own shortlist.** Plan 0086 built the capture,
ran it on three genres, and the reading named a defect **upstream of every candidate ranked
here**: the fold is indexed by onset-detector events, not musical beats — 1.73x / 1.35-2.10x /
1.76x detections per beat, wandering across 1x, 2x and 4x within a single track, against a
synthesized control that reads exactly 1.00. A second accent band changes *what* is measured per
event, not *what the events are indexed by*, so Alternative A could not have repaired it. Full
table in [Plan 0086](../plans/done/0086-the-downbeat-finds-a-cue-that-is-not-the-kick.md)'s
`Outcome`; the decision that follows is
[ADR-0109](0109-the-beat-clock-counts-onsets-not-beats.md).

**This is the outcome the Negative section priced and the Decision was written to produce** —
"a plan that went straight at the second accent band could plausibly ship a working improvement
sooner", and it would have shipped a plausible repair on top of a broken index. The phase bought
a redirection rather than a confirmation.

**The shortlist is deferred, not refuted.** The 2-periodic degeneracy this ADR named appears in
the data **only on backbeat rock/pop**, in the predicted direction: the top two alignment scores
sit closer to each other (top-2nd 0.0397) than the second and third do (0.0434), with `best`
flipping on 17.0 % of beats and `best == held` at only 23.3 %. It is weak and swamped by the
fold-unit defect, and it becomes answerable once Plan 0095 gives the fold a bar-scale unit.

**One claim in the Context did not survive its own measurement.** This ADR inherited Plan 0068's
6.79 % / 0.14 % genre split as the fact to be explained. It **did not reproduce**: all three
genres landed in a narrow 1.6-2.9 % band and backbeat rock/pop was not the worst. That may be
material the capture set lacked, or the two statistics may differ — 0068 measured shares of
audible 1 Hz rows, Plan 0086 shares of detected beats. Unresolved, and recorded here so the next
reader does not treat the 48x split as established.
