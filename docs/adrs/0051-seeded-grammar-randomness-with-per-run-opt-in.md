# ADR-0051 — `hash(x)` and `noise(x)` in the grammar, seeded per preset; an opt-in per-run seed that capture paths always pin

> **Status:** proposed
> **Date:** 2026-07-30
> **Related plan(s):** 0047-expression-randomness (R5)
> **Supplements:** ADR-0002/0020 (the grammar), NFR §6 (seeded visual randomness)

## Context

The grammar has no randomness of any kind, so authors fake non-repetition with sums of
incommensurate sines (four of them, with 170–331 s periods, in `attractor_dejong` alone) —
hand arithmetic standing in for one function call. NFR §6 never forbade this: it requires
visual randomness to be *explicitly seeded*, and a seeded hash is a pure function. The
starvation was an over-reading, named as wrong turn 5 in the visual-richness review.

The 2026-07-30 interview also chose the aggressive half: a preset may opt into a **per-run**
seed, so the same preset differs between app launches ("sometimes crazy"). That collides
head-on with the capture/golden harness, whose entire value is byte-reproducibility — unless
the collision is resolved the same way ADR-0045 resolved tiers: the live app varies, the
harness pins.

## Decision

We will add two pure functions: `hash(x)` — a deterministic uniform scatter in [0,1) of its
argument — and `noise(x)` — smooth value noise of a scalar argument (an author writes
`noise(time * 0.3)` for a wandering drift, `hash(beat_index)` for a per-beat lottery). Both
mix in a **per-preset salt** baked at load from the existing `[generator] seed` key
(reserved since Plan 0010, inert until now), so two presets with the same expression differ
and one preset is reproducible run to run. Expressions stay pure: the salt is load-time
constant, both functions are stateless maps of their arguments.

A preset may declare `seed = "random"`: the salt is then drawn **once at preset load** from
OS entropy in the live app — never per frame, never from wall-clock inside evaluation. Every
capture entry point (`shot`, goldens, `--report`, the behavioral gates) **forces a declared
numeric seed** (the file's number, or 0) exactly as they force the Floor tier (ADR-0045),
so the harness remains a pure function of its inputs and NFR §6 needs a clarifying sentence,
not a carve-out: *analysis* determinism is untouched; *visual* randomness is seeded, and
"random" names who supplies the seed, not whether one exists.

## Consequences

### Positive
- One function call replaces the incommensurate-sine idiom; `hash(beat_index)` gives
  per-beat variety no sine sum can fake; per-element `hash(index * 64)` scatters the
  spectrum scene without new machinery.
- Per-run variety exists for the presets that want to feel alive across sessions, at zero
  cost to the QA harness.

### Negative
- A `seed = "random"` preset's live look is not exactly its captured look (same statistics,
  different instance). The docs must say so where the key is documented, and the content
  lane must know captures verify the *distribution*, not the frame.
- `noise()` needs a smooth, cheap, allocation-free implementation on the per-frame path —
  a small fixed permutation, not a table allocated at eval time.

## Alternatives considered

### Alternative A — strictly seeded only (the recommendation)
Fully reproducible everywhere; rejected by the user as leaving "sometimes crazy" on the
table. Survives as the default — `seed = "random"` is opt-in, and everything else behaves
exactly as Alternative A.

### Alternative B — wall-clock or per-frame entropy in evaluation
Breaks expression purity and analysis determinism outright; never on the table (NFR §6,
ADR-0002). Named to mark the boundary: entropy enters once, at load, in the live app only.

### Alternative C — a `[random]` structural table with named distributions
More machinery (tables, kinds, ranges) for what two functions express; rejected as surface
without power.
