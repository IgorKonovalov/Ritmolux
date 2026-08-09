# 0089 — The library renews by replacement cohorts, never by a delete-all reset

> **Status:** accepted (2026-08-09 — the user took this decision explicitly, choosing it over
> the delete-all reset they had proposed).
> **Related plan(s):** [0075](../plans/0075-the-content-renaissance.md)
> **Relates to:** [roadmap-visual-richness R6](../roadmap-visual-richness.md) (the content
> renaissance), [ADR-0081](0081-the-content-lane-lands-presets-and-architect-curates-the-set.md)
> (the landing route the cohorts use), [ADR-0022](0022-build-time-preset-embedding.md) (why
> retiring a preset is deleting a file)

## Context

On 2026-08-09 the user asked whether the whole shipped preset library — 41 `.toml` files —
should be deleted so content can be authored "from scratch, something crazy and beautiful,"
free of the library's history. The instinct is well supported by the project's own record:

- **~55 % of the library is one template per family with different numbers** (the
  preset-surface survey, quoted in [roadmap wrong turn 6](../roadmap-visual-richness.md)).
- **Most of the set predates most of the engine's current power.** Linear-light HDR + bloom,
  normalized bands, phrase time, seeded `noise()`, shaped marks, the mandala interior, the IFS
  family, the depth levers, the fold-edge choice and the palette-driven backdrop all landed
  after the bulk of the library was authored.
- **Presets fossilize history.** [Backlog 0060](../design-backlog.md#0060--an-engine-fix-leaves-its-preset-side-workarounds-standing-and-only-a-header-comment-remembers-them)
  documents three shipped files that kept paying for engine defects after the defects were fixed.
- The presets authored geometry-first under the current craft principles measure **2-4x better
  on the animation metric** than the set they joined.

So a renewal is not in question — [R6](../roadmap-visual-richness.md) already commits to one.
What this ADR decides is the **mechanism**, because the obvious mechanism (delete everything,
then rebuild) has costs the instinct does not see:

- The shipped set is the **substrate of the behavioral gates** (`sanity`, `reactivity`,
  `animation`, `distinctness` all sweep it; only the golden drift guard is decoupled, by
  ADR-0023's frozen fixtures). An empty set makes the gates vacuous — the same gates
  [ADR-0081](0081-the-content-lane-lands-presets-and-architect-curates-the-set.md) rests
  direct-landing authorization on.
- **Active plans reference the set concretely**: Plan 0071's `occlude` default is a human look
  decision taken over the sixteen shipped skies; Plan 0064's sample grid renders on
  `attractor_lorenz`; Plan 0067 walks a candidate through the curation route.
- The app **ships** the embedded set; between "deleted" and "rebuilt enough" the product is
  hollow, and there is no release cadence under which that interval is invisible.
- The artistic freedom being bought is **not actually gated on deletion** — nothing forces a
  new preset to begin from an old file. The anchoring worry is an authoring-discipline
  problem, and `rm` is not a discipline.

One more force: the two engine capabilities that most enable the target look — transformed
feedback ([Plan 0046](../plans/done/0046-transformed-feedback.md), the tunnels/spirals/echo family)
and the symmetry stage ([Plan 0064](../plans/done/0064-the-symmetry-stage-and-the-banded-palette.md))
— are approved but not landed. A library authored from scratch *today* would be authored
against yesterday's engine, which is precisely the defect being escaped.

## Decision

**The library renews by replacement cohorts under a fresh-slate authoring rule, and is never
deleted wholesale.** Concretely:

1. **Cohorts, not a reset.** New presets are authored in cohorts of a few worlds each. A cohort
   lands through the [ADR-0081](0081-the-content-lane-lands-presets-and-architect-curates-the-set.md)
   route (gates + direct landing), and **each cohort retires a named list of old presets in the
   same commit series**. Retiring is deleting the `.toml` (ADR-0022: drop a file, it un-ships).
   The set is therefore always full, the gates always have a library to sweep, and every
   deletion is reviewable next to the world that replaces it.
2. **The fresh-slate rule.** A new world never begins by opening an old preset file. It begins
   from a reference look, a mechanism, or a blank file. Old files may be *consulted* for a
   measured ceiling recorded in a header — never used as a starting template.
3. **Sequencing.** Cohort authoring starts after the approved look-surface movers land
   (Plans 0071, 0064, 0046) and after Plan 0067 makes the gate worth trusting, so the new
   library is authored once, against the engine it will live on.
4. **The keep list is decided, not defaulted.** Presets authored under the current craft
   principles are candidates to survive; which ones actually do is a per-cohort curation call
   (the user's, from rendered evidence), not an exemption class.

[Plan 0075](../plans/0075-the-content-renaissance.md) carries the execution: the instrument
and documentation fixes the next library will be authored against, then the brief, then the
cohorts.

## Consequences

Positive:

- The behavioral gates keep a non-vacuous substrate at every moment, so the direct-landing
  authorization ADR-0081 rests on never lapses.
- Plans in flight that reference shipped presets are unaffected; no interim release ships a
  hollow app.
- Each retirement is a reviewed, named trade ("this world replaces these three clones") rather
  than an untraceable mass event, and `git log` keeps every retired header's measured knowledge
  findable.
- The old library keeps doing its one remaining job — being the measurement substrate — right
  up until each piece of it is individually outclassed.

Negative, and accepted:

- **The transition set is mixed** — new worlds ship alongside old clones for as long as the
  cohorts take. A delete-all reset would have had a cleaner "brand moment"; this has none.
- **The fresh-slate rule is discipline, not tooling.** Nothing mechanical prevents an author
  from templating off an old file; the rule lives in the plan text and the content lane's
  skill docs, and holding it is a review duty.
- **Some in-flight retune work will be partially discarded** — Plan 0071's Phase 5 retune and
  the routed backlog 0038/0058 content passes walk presets that later cohorts may retire. That
  cost is taken knowingly: those passes keep the shipped app good *during* the transition.
- The renaissance takes multiple sessions and has no single completion moment; it is finished
  when the last cohort's retirement list is empty, which requires someone to say so rather
  than a gate to fire.

## Alternatives considered

- **A — Delete all presets, then rebuild from scratch** (the user's opening proposal). Rejected
  because it guts the behavioral-gate substrate and the shipped app for the whole rebuild
  interval, strands three active plans that reference the set, and buys nothing the fresh-slate
  rule does not — the freedom sought is freedom from *templating*, not from the files' existence.
- **B — Keep tuning the existing set in place** (the status quo, extended). Rejected because the
  record shows its ceiling: ~55 % template clones, audio bound to luminance across the old
  cohort, and workarounds that outlive the defects they dodged. Retunes preserve history;
  the mandate is to escape it.
- **C — Author the fresh library now, before Plans 0046/0064 land.** Rejected because it
  recreates the defect being escaped: a library composed against a weaker engine, dated the
  day the look surface moves. The delay is short (both plans are approved) and the authoring
  happens once instead of twice.
