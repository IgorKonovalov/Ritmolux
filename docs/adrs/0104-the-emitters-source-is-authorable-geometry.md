# ADR-0104 — the emitter's source is authorable geometry, and the pool can start warm

> **Status:** proposed
> **Date:** 2026-08-13
> **Related plan(s):** [0090 — the emitter's source moves](../plans/0090-the-emitters-source-moves.md)
> **Supplements:** [ADR-0057](0057-emitter-scene-analytic-ballistics-seeded-individuation.md),
> [ADR-0091](0091-the-animation-gate-scores-motion-against-the-figures-footprint.md)

## Context

The emitter's source geometry is two hardcoded facts. `SOURCE_Y = -1.12`
(`core/src/render/scenes/emitter.rs:91`) puts the source line just below the visible frame — its
docstring gives the reason, *"so an upward-launched object rises into shot rather than appearing in
it"* — and the line's width is `source_half_width: self.aspect` (`emitter.rs:898`), the full frame
half-width. `presets/README.md` states the limit outright: *"There is also no positionable source:
the line spans the frame width at `y = -1.12` and cannot be moved or narrowed. A look that wants a
point fountain or an off-centre jet is engine feedback, not a preset."*

**Two independent wants have hit it**, both recorded in [backlog 0068](../design-backlog.md):

- **A point fountain / off-centre jet** — the author-facing ask the README paragraph anticipates.
- **A slow-drift field.** The entry measured the emitter for a starfield and found it unusable *for
  geometric reasons rather than taste*: a star must travel 2.12 world units to cross the frame, so a
  drift slow enough to read as a sky (~0.85 units/s) needs ~2.5 s to fill it, while **every
  behavioral gate captures 30 frames at 1/60 s — 0.5 s**. The measured emitter draft reported cover
  `0.013` and `0.000` on all four bands. Speeding it to the ~4.3 units/s the geometry demands is a
  rising shower, not a sky.

**The compromise shipped, which is what makes this a demonstrated want rather than a hypothesis.**
`presets/emitter_perseids.toml` exists on `system = "emitter"` with `launch_speed = 2.6` — the fast
shower the entry predicted would be the only reachable form. The entry's option 1 (per-mark variation
on the swarm) was delivered by [Plan 0077](../plans/done/0077-the-quiet-sky.md); option 2, the
source, stayed open and is the half this ADR decides.

**The internals are already the right shape.** `source_half_width` is a real field on `Spawn`
(`emitter.rs:357`) that happens to be assigned `self.aspect` unconditionally, and the spawn site
(`emitter.rs:505`) already multiplies a unit draw by it. Nothing needs restructuring; the question is
which knobs to expose, what their defaults mean, and whether a source may sit **inside** the frame —
which trades the no-pop guarantee for the only route to a gateable slow look.

**And there is a second warm-up the geometry does not fix**, found while grounding this decision. The
pool starts empty (`started = false`, `next_spawn = time` on the first `step`) and fills at
`spawn_rate`, so a world's *population* ramps toward `rate * lifetime` regardless of where the source
sits. Perseids' own numbers: `spawn_rate ~ 200/s` and `lifetime = 2.8 s` give a steady state of ~560
objects, well under the 2,000-object Floor pool — and at the gate's 0.5 s the pool holds ~100 of
them, **about 18 %**. So moving the source into the frame removes the *travel* warm-up and leaves the
*population* warm-up untouched. A slow emitter world is still ungateable without addressing it.

## Decision

**The source becomes two authorable scalars, a spawn fade makes an inside-frame source usable, and
the pool can be asked to start in steady state.** Four params on `emitter`, every default an exact
arithmetic identity so no shipped preset and no baseline moves:

| param | default | meaning |
|---|---|---|
| `source_y` | `-1.12` | the source line's world `y`. Today's constant, now a value. **May sit inside the frame.** |
| `source_width` | `1.0` | the line's half-width **as a fraction of the frame's**, so `source_half_width = aspect * source_width`. `0` is a point source. |
| `spawn_fade` | `0` | fraction of an object's lifetime over which its brightness ramps from 0. `0` is today: full brightness at spawn. |
| `prewarm` | `0` | at scene start, back-date `prewarm * lifetime` seconds of spawns so the population begins at steady state instead of ramping. |

Three properties the shape is chosen for:

- **`source_width` is fractional, not absolute world units.** That is what makes the default an
  identity — `aspect * 1.0` is bit-for-bit `self.aspect` — and it keeps the aspect where
  [ADR-0037](0037-internal-grid-is-a-resolution-not-a-shape.md) says it belongs: an absolute width
  would have to be reconciled against the aspect by the author instead, and a source that spans
  "the frame" would then be a different number on every display.
- **A point source falls out of the same param at `0`** rather than being its own concept. The spawn
  site already multiplies a unit draw by the half-width, so zero collapses it exactly.
- **`prewarm` is a param and not a behaviour change.** Defaulting it on would start every shipped
  emitter world full, move the one `emitter.png` baseline, and break
  `a_spawn_rate_on_onset_bursts_and_then_idles`, which asserts an *empty* frame at its lead peak.
  Back-dating is exact rather than approximated: the path is closed-form in `t - t0` and `death_time`
  is derived from `t0`, so a back-dated object is indistinguishable from one that spawned then, and
  the draw stays seeded (NFR §6) with no clock read.

**An inside-frame source is legal, and the pop is answered with the fade rather than with a clamp.**
Refusing to let `source_y` enter the frame would preserve today's guarantee absolutely and leave the
slow-drift half of backlog 0068 permanently unreachable, which is the wall that routed a look out of
a cohort. `source_y` is still clamped — but by *correctness*, to inside the retirement bound
(`RETIRE_MARGIN = 1.6` times the frame half-extents), because a source outside it spawns objects
whose `exit_time` has already passed and churns the pool against itself.

## Consequences

**Positive**

- **Both wants in backlog 0068 option 2 become preset-reachable**, and the README paragraph that
  routes them to engine feedback is retired rather than reworded.
- **The emitter becomes gateable for a slow look** — `source_y` inside the frame kills the travel
  warm-up and `prewarm` kills the population warm-up, which together are the whole of what the 0.5 s
  capture could not see. **Neither the gate's capture length nor its floors move**, which backlog 0068
  named as the wrong answer and ADR-0091 would have to relitigate.
- **Zero pixels move on the way in.** Four defaults, four exact identities; the one emitter golden
  baseline and its three fixtures are unaffected by arithmetic rather than by tuning.
- **`spawn_fade` is worth having beyond the pop.** A ramp on a short-lived object is a *soft* spark,
  which the scene has no way to express today at any `brightness`.

**Negative**

- **Four params on a scene that already carries 26**, and three of them are inert in every shipped
  world. That is the honest surface cost; what limits it is that each is a scalar on machinery that
  already exists, and none adds a state machine.
- **A visible spawn pop becomes authorable.** `spawn_fade` makes it *avoidable*, not impossible, and
  nothing validates the pair — an author can put the source mid-frame at `spawn_fade = 0` and get
  exactly the artifact `SOURCE_Y`'s docstring was written to prevent. Coupling the two with a
  validation rule was considered and rejected below.
- **`prewarm` changes what "scene start" means**, and a preset switching *into* a prewarmed emitter
  appears fully populated in one frame. That is the intent for a sky and would be wrong for a
  cascade, which is exactly why it is a param — but the two readings live one number apart.
- **The gate story gets more subtle, not simpler.** After this, a green `anim` on an emitter world
  says something different depending on `prewarm`, and the gate cannot see which. `docs/capturing.md`
  gains another line in the section that already carries three such caveats.

## Alternatives considered

- **A — expose `source_width` only, leaving `SOURCE_Y` fixed.** The cheapest option and it delivers
  the point fountain. Rejected because it leaves the *other* want — the one with a measured
  casualty — exactly as unreachable as it is today: a narrow source below the frame still needs 2.12
  units of travel, so the gate still sees an empty frame.
- **B — a named source shape (`line | point | ring | area`), in ADR-0084's silhouette style.** More
  expressive and stylistically consistent with the mark roster. Rejected as speculative surface:
  `ring` and `area` have no demonstrated want, each needs its own spawn distribution, and the two
  scalars already reach every shape anyone has asked for. It stays available if a ring is ever wanted.
- **C — clamp `source_y` below the visible frame.** Preserves the no-pop guarantee as an invariant
  rather than a default. Rejected on the gate: it is the decision that keeps the emitter unusable for
  any slow look, and this project has already paid for that once by routing a world out of a cohort.
- **D — make an inside-frame `source_y` legal only when `spawn_fade > 0`.** Tempting, and it would
  make the pop unauthorable. Rejected because the engine has no precedent for a cross-param
  validation rule, the preset surface is deliberately a flat set of independent scalars, and a rule
  like this fails badly under `[smoothing]` — an eased `spawn_fade` passing through zero would make a
  legal preset briefly illegal. Documentation carries it instead.
- **E — raise the behavioral gates' capture length so a slow emitter fills.** Rejected, and backlog
  0068 rejected it first, in the entry's own words: the gates are 0.5 s by design, and *"a preset that
  needs 2.5 s of warm-up to look like anything is also a preset that looks like nothing for the first
  2.5 s of a live show."* `prewarm` attacks the warm-up rather than the instrument.
- **F — `source_width` in absolute world units.** Reads more naturally in isolation. Rejected because
  the default then depends on the aspect, so "unchanged" stops being an arithmetic identity and
  becomes a per-display coincidence — and it hands the author an aspect reconciliation that ADR-0037
  says the engine should have already done.
