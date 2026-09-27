# ADR-0253 — A retirement may land ahead of its replacement when a walk convicts it

> **Status:** proposed
> **Date:** 2026-09-27
> **Related plan(s):** [0232](../plans/0232-the-library-is-walked-cut-and-refilled.md)
> **Amends:** [ADR-0089](0089-the-library-renews-by-replacement-cohorts.md) (Decision item 1)

## Context

[ADR-0089](0089-the-library-renews-by-replacement-cohorts.md) set how the library renews: in cohorts,
where **each cohort retires a named list of old presets in the same commit series**. A preset leaves
only when a new world arrives to take its place. Its reasons were about a delete-all reset: an empty
set makes the behavioural gates vacuous, plans in flight name shipped presets, and a hollow interim
release ships.

The library that rule governed had 41 presets. On 2026-09-27 it has 121, and none has ever left
under the rule. The set grew by addition alone. `attractor_*` is 20 files and `fragment_*` is 14.
ADR-0089's own Context found that *"~55 % of the library is one template per family with different
numbers"*, and that figure has not been re-measured since. The owner's stated aim in backlog 0256 is
**to ship less but better**. Asked directly, the owner chose to cut first and refill afterwards over
holding the count steady by pairing every cut with a replacement.

Strict pairing gets in the way of that aim in a specific way. A near-duplicate or a weak preset can
only leave once someone has authored its successor. A successor for a duplicate is not needed at all,
since the other half of the pair already covers that look. So under strict pairing the cheapest cut
in the library costs a whole authoring sitting.

## Decision

**A preset may be retired with no replacement when a human walk of the running app convicts it**,
recording it as a duplicate of a named survivor or as not worth shipping, with a one-sentence reason
in the plan that carries the walk. Retirement is still deleting the file (ADR-0022). A pure cull is
bounded by what kept ADR-0089's gates meaningful: **every system family keeps at least two shipped
presets and at least two declared representatives**. Those are the floors `distinctness` and
`every_family_carries_at_least_two_representatives` already assert. A cut that would take a family
below either floor is not a cull. It is a question about the system, and it goes to `architect`.
Replacement cohorts stay the way new content arrives. They no longer have to retire anything.
ADR-0089's fresh-slate rule and its "the keep list is decided, not defaulted" rule stand as written.

## Consequences

### Positive
- A duplicate leaves at the price of a verdict, not an authoring sitting.
- The gates keep a non-vacuous substrate, because the floors are the ones the gates already assert.
- Every retirement is still reviewable and named. The conviction and the survivor it defers to are
  written down next to it.

### Negative
- **The shipped count can drop between releases.** An operator who liked a retired preset loses it
  on upgrade with no in-app notice. Their mark for it in `marks.toml` stays in the file and does
  nothing (ADR-0228). A retired preset can come back from `git log`, but nothing surfaces that.
- **The conviction rests on one person's eye.** Similarity has a report and quality has none
  (backlog 0256). A walk-based cull is exactly as good as the walk, and nothing checks it
  afterwards.
- **The floor is a count, not a quality bar.** A family can end at two weak presets and still pass.
  Whether it should keep shipping is a system question this ADR does not answer.

## Alternatives considered

### Alternative A — Strict pairing, as ADR-0089 wrote it
Every cut waits for a successor in the same commit series. It lost because a duplicate needs no
successor: its twin already covers the look. Pairing turns the cheapest cut into the most expensive
one, and the owner declined to hold the count steady.

### Alternative B — A similarity threshold retires presets automatically
Cut whichever pair `distinctness` flags above a threshold. It lost because backlog 0256 already
records the owner's order: evidence from a person first, then any mechanism. It also lost because
similarity is the half that can be measured. A threshold would retire the wrong member of a pair
and treat a number as a curation verdict.

### Alternative C — Retire into a `presets/retired/` directory instead of deleting
This keeps the file one `git mv` from shipping again. It lost because two held-out directories
already exist and each has one meaning: `presets/pending/` is "approved, held back by an engine
gap", and `presets/proposed/` is "authored, awaiting the owner's verdict". A third would hold
content nobody intends to ship, and it would rot beside two that hold content somebody does. `git log`
already keeps every retired header's measured knowledge findable, which ADR-0089 counted as a
positive.
