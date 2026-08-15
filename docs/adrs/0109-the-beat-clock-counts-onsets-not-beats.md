# ADR-0109 — the beat clock counts onsets, not beats, and Layer 2 gets its own grid

> **Status:** proposed
> **Date:** 2026-08-15
> **Related plan(s):** [0095 — the downbeat fold gets a musical beat](../plans/0095-the-downbeat-fold-gets-a-musical-beat.md),
> measured by [0086 — the downbeat finds a cue that is not the kick](../plans/done/0086-the-downbeat-finds-a-cue-that-is-not-the-kick.md)
> **Supplements:** [ADR-0050](0050-downbeat-and-phrase-tracking-with-confidence-fallback.md),
> [ADR-0097](0097-the-downbeat-cue-is-chosen-against-per-beat-evidence.md)

## Context

[ADR-0050](0050-downbeat-and-phrase-tracking-with-confidence-fallback.md) built the time
vocabulary in two layers. Layer 1 is "`beat_index` — a **monotone beat counter** from the
existing tracker"; Layer 2 folds the accent history "over the four candidate beat-1
alignments of a 4/4 hypothesis". Every later document reasons from that sentence.
[ADR-0082](0082-the-downbeat-gate-holds-and-the-estimator-is-diagnosed-first.md) measured the
estimator publishing on 6.0 % of audible time and named the accent feature as the cause;
[ADR-0097](0097-the-downbeat-cue-is-chosen-against-per-beat-evidence.md) refused to build on
that inference and spent a phase capturing the per-beat decomposition first.

**That phase falsified a premise none of the three ADRs had thought to state.** A detected
beat is not a musical beat. `beat_index` is `beats_seen - 1` (`core/src/dsp/tempo.rs:122`),
`beats_seen` increments unconditionally on the onset detector's flag (`tempo.rs:107-110`), and
that flag is a bare adaptive threshold on spectral flux — `flux > mean + 1.5 * std` with a
96 ms refractory and **no tempo gating whatsoever** (`core/src/dsp/onset.rs:70-73`).

Measured through the live app on three genres, 240 s each (Plan 0086 Phase 2):

| | techno | instrumental hip-hop | backbeat rock/pop |
|---|---|---|---|
| detections per musical beat | 1.73x | 1.35x-2.10x | 1.76x |
| per-row ratio, p10 / med / p90 | 0.99 / 2.03 / 4.83 | 0.68 / 1.95 / 3.67 | 0.95 / 1.92 / 4.43 |
| rows near 1x / 2x / 4x | 23 / 33 / 12 % | 19 / 30 / 3 % | 30 / 24 / 4 % |
| publish rate | 2.89 % | 1.59 % | 2.27 % |

The control that makes this a measurement rather than a suspicion is in the test suite: a
synthesized clip with exactly one transient per beat reads **1.00** and a tempo estimate of
exactly 120.0 BPM. The instrument reads 1.00 when the material is 1.00.

So `beat_index % 4` spans well under a bar, at a ratio that is **not a stable integer within a
single track**. A bar-locked accent precesses across all four alignments instead of
accumulating in one — which predicts every other number in that table: the flat score ladder,
the modest `effect_raw`, the median `effect_corrected` of exactly 0.0000, and the 1.6-2.9 %
publish rate on all three genres.

**The obvious repair is contaminated.** The tempo tracker does hold a beat phase, but
`tempo.rs:107-110` hard-resets it to `0` on every detected onset, so it is yanked 1.7-2.1x per
beat by the same detector. Only `bpm` — the mean-subtracted autocorrelation of the onset
envelope (`tempo.rs:102`) — is independent of the over-firing flag. And that estimate is itself
octave-unstable: across the same three runs it reported a p10 of 64.0 against a 128.0 median,
and a p90 of 200.9 against a 100.2 median, i.e. hitting both `MIN_BPM`/`MAX_BPM` search bounds.

**The blast radius reaches past the estimator, into what the project tells authors to write.**
Both authoring references teach arithmetic that this measurement makes false:

- `presets/README.md` — *"`beat_index` and `time_since_beat` are unconditional: always
  tracked, **always meaning what they say**"*, and *"Write `mod(beat_index, 16)` when you mean
  four bars of four."*
- `docs/presets.md` — *"`mod(beat_index, 4)` for 'every 4th beat'."*

The first of those is the sharpest statement of the falsified premise anywhere in the repo, and
it is load-bearing: it is the sentence that sends an author to Layer 1 as the trustworthy one
after the same page has just explained why Layer 2 is not.

Six shipped presets do beat-count arithmetic against it (`attractor_valentine` `mod 32`,
`attractor_torusknot` `mod 16`, `attractor_thomas` `mod 24`, and `attractor_dragon`,
`curve_nightbloom`, `fragment_vitrail` on `floor(beat_index / 4)` — the last commented "four
beats"). Each re-fires roughly twice as often as authored. **Nothing in the repo could have
caught this**: a periodic re-cut at the wrong period still renders as a periodic re-cut, every
gate passes, and no test compares a preset's intended musical period against its actual one.

## Decision

We will treat **`beat` and `beat_index` as what they measurably are — a transient flag and a
transient counter — and give Layer 2 its own bar-scale grid** that no other consumer sees.

Concretely: the downbeat tracker stops folding over `beat_index % BEATS_PER_BAR` and folds
instead over a **tempo-derived bar position**, built inside the analysis path from the
autocorrelated `bpm` plus a phase accumulator that is *not* reset by every onset. `beat`,
`beat_index` and `time_since_beat` keep their present behaviour bit for bit, so no preset's
flash timing moves and no baseline re-blesses on their account.

**Settling the tempo estimate is part of this decision, not a follow-on.** A grid built on an
octave-jumping rate inherits the jump, and the measurement above shows the rate jumping octaves
on all three genres. The repair is not credible without it.

**The authoring docs are corrected as part of the same work**, because they currently instruct
authors to build bar arcs on a counter that does not count bars, and every day that stands is
another preset authored against it.

## Consequences

### Positive

- **Layer 2 becomes able to be right.** The fold gets a unit that is a bar, which is the
  precondition for any accent-feature question — including the one ADR-0097 was written to
  answer and could not.
- **The blast radius is bounded by construction.** Presets react to `beat` for flashes and that
  is exactly what it will keep doing. The only visible change is that `beat_in_bar`,
  `bar_index` and `bar_phase` start meaning what their names say.
- **The measurement instrument already exists.** Plan 0086 Phase 1 shipped `--downbeat-log`,
  and Phase 2's three captures are the before-baseline any repair is read against. This is the
  first estimator change in the project's history that can be measured rather than argued.
- **The two authoring references stop teaching a false idiom**, and the six mis-scaled presets
  get named rather than left for the next author to trip over.

### Negative

- **`beat_index` keeps a name that describes it badly.** Renaming it is a grammar break every
  shipped preset would have to be edited through, and this decision declines to pay that for
  accuracy of naming alone — so the docs carry a correction that the name itself contradicts.
  That is a real, ongoing cost, and it is chosen.
- **Two clocks now exist in the analysis path** — the transient stream and the bar grid — and a
  reader has to know which one a variable comes from. The alternative was one clock that is
  wrong for one of its two jobs.
- **Tempo octave stability is genuinely hard**, and it is now on this plan's critical path. If
  it cannot be settled the fold has no trustworthy grid and the plan stops there, having spent
  its cost on a diagnosis.
- **The six presets' arithmetic stays as-authored.** Because `beat_index` does not change, a
  preset asking for `mod 16` keeps getting ~8 musical beats; what changes is that its comment
  and the docs stop claiming otherwise. Re-tuning them to a musical period is content work and
  is not in this decision.

### Neutral

- The C ABI does not move. Analysis diagnostics are native-only (ADR-0052) and none of this
  crosses the boundary; `LMV_ABI_VERSION` stays at 4.
- `CONFIDENCE_THRESHOLD` still does not move, for ADR-0082's unchanged reason: the gate keeps
  its meaning while the estimator changes underneath it.

## Alternatives considered

### Alternative A — tempo-gate the beat flag at the source

Make `beat` fire only on grid-aligned transients, so `beat` and `beat_index` mean musical beats
everywhere and every preset's bar arithmetic starts working. **Rejected on blast radius.**
`beat` is a published grammar variable that the shipped library reacts to for flashes and
accents; re-timing it would re-time the whole library's transient response, force a re-bless of
the golden suite, and require a content pass over every preset that binds it — to fix an
estimator nothing outside Layer 2 depends on. The naming stays wrong; the pixels stay right.

### Alternative B — split the two, keeping `beat` and fixing `beat_index`

Leave `beat` a transient flag but make `beat_index` a true musical-beat counter off the tempo
grid. Tempting, because it repairs the documented idiom and the six presets for free.
**Rejected because it is a silent semantic change to a shipped grammar variable**: the same
expression keeps parsing, keeps running, and starts meaning something else, with no error and
no diff in any preset. Presets built on `hash(beat_index)` would re-roll on a different
schedule and the six with `mod` arithmetic would change period — visible changes nobody asked
for, arriving without a single preset edit to review. If that repair is wanted it deserves its
own decision and its own content pass, not a side effect of this one.

### Alternative C — accept it and correct only the documentation

The cheapest honest option, and a real one: say plainly that `beat_index` counts transients,
retract the bar-arithmetic idiom, and leave Layer 2 as the fallback counter it effectively is.
**Rejected because it forecloses the capability rather than deferring it.** ADR-0050's Layer 2
was designed for phrase-scale structure, four plans have now been spent measuring why it does
not publish, and this ADR is the first to name a cause that is both upstream of the accent
feature and repairable. Documenting the defect at the moment its cause is finally understood
would be the worst-timed retreat available. (The documentation half is taken regardless — it is
part of the Decision, not an alternative to it.)

### Alternative D — build the fold on the existing tempo phase

Use `BeatClock::bar` (the tracker's 0..1 phase) as the bar-scale index, which needs no new
state. **Rejected on inspection of what that phase is**: `tempo.rs:107-110` resets it to zero on
every detected onset, so it carries exactly the contamination this ADR exists to remove. It is
a beat phase snapped by transients, not a grid.

## Notes

**What this ADR does not claim.** Plan 0068's 6.79 % / 0.14 % genre split **did not reproduce**
in Phase 2 — all three genres landed in a narrow 1.6-2.9 % band and backbeat rock/pop was not
the worst. Either the split is a property of material that capture set did not contain, or the
two measurements are not measuring the same statistic (0068's is a share of audible 1 Hz rows,
Phase 2's a share of detected beats). This ADR does not resolve that, and the fold-unit finding
does not depend on it.

**ADR-0097's degeneracy story survives.** On backbeat rock/pop, and only there, the top two
alignment scores sat closer to each other than to the rest (top-2nd 0.0397 against 2nd-3rd
0.0434), with `best` flipping on 17.0 % of beats and `best == held` only 23.3 % — the estimator
oscillating between two candidates, exactly as a kick on 1 and 3 predicts. It is weak and it is
swamped by the fold-unit defect, but it is present, on the predicted material, in the predicted
direction. It becomes answerable once the fold has a bar-scale unit, which is why ADR-0097's
Alternative A is deferred rather than refused.
