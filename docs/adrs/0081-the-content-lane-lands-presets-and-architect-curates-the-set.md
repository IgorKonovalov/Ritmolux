# ADR-0081 — The content lane lands presets; `architect` curates the shipped set

> **Status:** accepted (Plan 0067, closed 2026-08-09 — see the Outcome section)
> **Date:** 2026-08-04
> **Related plan(s):** [0067](../plans/done/0067-the-curation-route.md)
> **Supplements:** [ADR-0017](0017-preset-author-skill-lane.md)

## Context

[ADR-0017](0017-preset-author-skill-lane.md) created the `preset-author` lane and drew its boundary
at authoring: the lane composes presets, and **`dev` embeds a curated preset** into the shipped set.
That boundary was drawn for a mechanical reason — at the time, embedding a preset meant editing Rust
in two coupled spots, which is `dev`'s work by definition.

[ADR-0022](0022-build-time-preset-embedding.md) removed the premise. `core/build.rs` globs
`presets/*.toml`, so shipping a preset is now *committing a file*. The stated justification for the
boundary has not been true for some time, and the `architect` skill has carried "an ADR-0017
supplement someone owes" in its own text since — a boundary standing on a retired reason, which is
the worst kind, because nobody can tell whether it is load-bearing.

**There is now a concrete case waiting on it.**
`%APPDATA%\light-music-visualizer\presets\chthonic_coral_oracle.toml` — a reaction-diffusion preset
composing Pearson-regime drift on bass, beat-stamped `inject` growth, trails and a breathing fold —
**has never been tracked in git**. It is the preset that raised
[backlog 0001](../design-backlog-archive.md#0001--reaction_diffusion-reaches-only-2-of-the-5-plan-0018-composite-levers)
on 2026-07-24, which became [ADR-0026](0026-full-composite-coverage-fullscreen-scenes.md) and
[Plan 0025](../plans/done/0025-full-composite-coverage.md). The preset that motivated a whole plan
never came back into the repo, and the levers it asked for landed without the look that asked for
them ever shipping. Six weeks outside the repo is what a missing route looks like.

The forces are not symmetric. Authoring throughput wants the shortest path from a good look to a
committed file. Set quality wants somebody other than the author deciding whether the *library*
needs another one — the same fresh-context argument that keeps `dev` from reviewing its own work.
And a third force cuts against the cheap answer: **the five preset gates do not run the analyzer.**
`sanity.rs`, `reactivity.rs`, `animation.rs`, `distinctness.rs` and `golden.rs` synthesize
`AnalysisFrame` values directly; only `beat.rs`, `chain.rs`, `dsp.rs` and `saturation.rs` push PCM.
So "the suite is green" currently means *the renderer did something with numbers we made up*, which
is a weaker authorization than it sounds like.

## Decision

We will let `preset-author` **commit curated presets to `presets/` directly**, gated on the
behavioral suite passing, and we will make **`architect` the curator of the shipped set** — deciding
periodically what stays, what is retired, and whether the library has gained a look or gained a
duplicate. `dev` leaves the content loop entirely; it keeps engine work, and it stops being a
courier for a file copy.

The curation pass is **owed at the close of any plan that touched `presets/`**, and its output is a
one-line verdict in that plan's close notes. Attaching it to an existing ritual is deliberate: this
project has already learned that a periodic architect duty with no hook gets skipped (the version
bump sat still across five plans), and a cadence nobody can name is a cadence nobody runs.

## Consequences

### Positive

- **The route exists.** A strong preset can ship the day it is judged strong, by the lane that
  judged it, without a handoff whose only remaining content is `git add`.
- **The boundary stops resting on a retired fact.** Whatever the split is, it is now justified by
  what it protects (the composition of the set) rather than by a build-system detail that changed
  two months ago.
- **Set-level judgement gets an owner.** "Does the library need this?" is a different question from
  "is this preset good?", and it has never had anyone assigned to it. Retirement — which this
  project does periodically and always ad hoc — becomes a named duty rather than a side effect of
  someone noticing.
- **`dev` sessions get shorter.** Engine plans stop carrying preset-embedding phases that exist
  only because of the old boundary.

### Negative

- **The shipped set can now grow between curation passes**, and growth is the failure mode this
  library has actually had (the rose family reached eleven presets before a pass cut it to five).
  A curation pass hooked to plan closes is a real hook, but it is still a person remembering.
- **The gate authorizing all this is weaker than it reads.** Green means the renderer produced a
  plausible frame from synthesized analysis. It does not mean the preset reacts to music. Until at
  least one preset gate drives real PCM through the analyzer, "gated on the suite" is a partial
  claim and should be stated as one — [Plan 0067](../plans/done/0067-the-curation-route.md) Phase 1
  exists to stop that from staying true.
- **The lane self-approves the individual file.** That is the trade: per-file judgement moves to
  the author, set-level judgement moves to `architect`. A bad preset can ship and live until the
  next curation pass. The mitigation is that a preset is cheap to retire and an engine change is
  not — the asymmetry is what makes this boundary affordable and it should be said out loud.
- **`architect` gains a recurring duty in a role that is otherwise event-driven.** The close-ceremony
  hook is the smallest version of it; if curation starts being skipped, the answer is a narrower
  trigger, not a broader one.

### Neutral

- ADR-0017's other boundaries are untouched: `preset-author` still never writes engine Rust, and an
  engine gap still routes to `architect` then `dev`. This supplement moves exactly one thing — who
  commits a `.toml`.

## Alternatives considered

### Alternative A — Keep `dev` as the gate, re-justified on fresh-context grounds

Drop the retired build-system reason and keep the handoff on the argument that the lane which
authored a preset should not also approve it — the same principle that keeps `dev` from reviewing
itself. Rejected because the analogy does not hold: `dev`'s self-review ban exists because a fresh
*context* catches drift, and `dev` reading a preset brings no content context at all — it cannot
judge whether a look is good or whether the set needs it, so the handoff buys a signature rather
than a review. A gate that cannot evaluate what it is gating is ceremony.

### Alternative B — The lane lands presets with no curation pass at all

Simplest, and it is what "gated on the tests" would mean if taken literally. Rejected because the
tests are per-preset and the risk is per-*set*: five green presets that are palette swaps of each
other pass everything and make the library worse. `distinctness.rs` catches the crudest version of
this and nothing catches the interesting version.

### Alternative C — A standing cadence (weekly, or every N presets) instead of a close-ceremony hook

More predictable in principle. Rejected on this project's own evidence: the one architect duty with
a standing cadence and no hook — the version bump — was missed five plans running. Hooks that ride
an existing ritual get run; calendars do not.

## Notes

- The concrete case this was written against is
  [backlog 0056](../design-backlog.md#0056--a-user-authored-preset-has-been-living-outside-the-repo-for-six-weeks-and-it-is-a-curation-candidate-the-boundary-has-no-route-for),
  which also records two rot findings in that file (`bar` is no longer a variable; `kaleido_order`
  is eased through non-integer values) — read them before rendering it, and render it before
  judging it.
- The gate-strength problem is not new here and is not this ADR's to solve; it is recorded so that
  "gated on the suite" is never quoted as stronger than it is.

## Outcome — 2026-08-09, at Plan 0067's close

Accepted as decided; the boundary moved and both skills describe it. Two things this ADR recorded
have changed, and one of them was wrong when written.

**The gate-strength Negative is discharged, and it should stop being quoted as open.** It read
"until at least one preset gate drives real PCM through the analyzer, 'gated on the suite' is a
partial claim". Plan 0067 Phase 1 made `reactivity` do exactly that: PCM through `Analyzer` via
`Renderer::capture_audio`, with a non-vacuity test proving a preset whose only band binding is
deleted now **fails**. The claim is no longer partial in the way this bullet meant. What replaces it
is narrower and is written down in [`docs/capturing.md`](../capturing.md): **one** of the five gates
sees audio, the other four synthesize by design, and green means *the preset reacts to at least one
band* — not that it reacts *well*, since `reactivity` compares a driven band against silence.

**The `bar` claim in the Notes is false and is struck.** This ADR repeated Plan 0067's error that
`bar` "is no longer a variable". It is `VAR_NAMES[5]` and is live: the **beat** phase in `[0, 1)`, a
misnomer kept for compatibility, with `bar_phase` — which ADR-0050 *added alongside* it — being the
genuine bar position. The Coral Oracle pass verified this against the code and left the binding as
authored. Only the `kaleido_order` easing was real rot. The lesson is the one this project keeps
relearning: a claim about the current surface, repeated from a plan into an ADR, inherits the plan's
error and outlives it.

**The recurring-duty Negative stands and has one data point.** Step 3b ran twice on the day it
landed — at [Plan 0064](../plans/done/0064-the-symmetry-stage-and-the-banded-palette.md)'s close and
at this one. Both found a clean set. The one adjustment it needs is mechanical rather than
structural: the workaround grep as specified returns dozens of hits, because this project cites
plans and ADRs as *rationale* far more often than as dodges, so the sweep needs a narrowing pass
before its output is readable.
