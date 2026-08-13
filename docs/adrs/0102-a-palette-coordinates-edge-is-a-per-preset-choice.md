# ADR-0102 — a palette coordinate's edge is a per-preset choice

> **Status:** proposed
> **Date:** 2026-08-13
> **Related plan(s):** none — deliberately. Recorded now, built when a want arrives.
> **Supplements:** [ADR-0086](0086-the-backdrop-colours-through-the-preset-palette.md),
> [ADR-0088](0088-the-ifs-colours-by-distance-from-its-own-skeleton.md)
> **Precedent:** [ADR-0061](0061-kaleidoscope-edge-treatment-is-a-per-preset-choice.md)

## Context

The palette LUT is a 256x1 texture read through **one** sampler, and that sampler wraps:
`address_mode_u: wgpu::AddressMode::Repeat` (`core/src/render/palette.rs:332`). Its own docstring
gives the reason — *"so a hue rotation past the gradient edge wraps like the cosine's periodic
wheel"* — and the repeat is load-bearing and documented in six places in
[`docs/preset-palettes.md`](../preset-palettes.md): the gradient repeats past its ends (line 101),
`color_center` is cyclic and wraps rather than clamps (149), `depth_hue` rides a coordinate the LUT
repeats (202, 209), a banded palette's steps wrap (521), and a `color_span` above 1 wraps
deliberately (575). None of that is a mistake and none of it should change.

**But not every contributor to that coordinate is cyclic.**
[Backlog 0075](../design-backlog.md) measured the consequence on the IFS's root channel.
`root_tint` is an **anchored** coordinate term — [ADR-0088](0088-the-ifs-colours-by-distance-from-its-own-skeleton.md)
reasoned the anchoring from the measured distribution and it is correct — so it only ever pushes
*up* the ramp, which on the shipped palettes spends the bright end that is already occupied. The
documented escape is to drive it **negative**, which spends the dark end instead and reads well on
the fern. At `root_tint = −0.55` a **bright cream speckle appears in the middle of the figure, in
the region that should be darkest**: the coordinate crosses zero and the sampler repeats, wrapping
the darkest points to the ramp's brightest stop. The arithmetic is known — the fern's coordinate
floor goes negative at about `root_tint = −0.38` — and `presets/README.md` documents the negative
direction and the wrap as of Plan 0074's close, so an author can currently avoid it by computing it.

**The reason this is a decision and not a patch is that there is exactly one coordinate.** It is a
sum: `hue_center ± hue_spread/2`, plus `map_tint`, plus the root term, plus whatever a scene adds,
resolved to a single `f32` and fetched once. Some contributors are angles, where wrapping is the
*correct* behaviour and clamping would be a bug; others are distances and depths, where wrapping
turns a continuous quantity into a discontinuity at its own floor. **A sum cannot have two
addressing behaviours**, so no per-param classification can express this, and the property is not a
property of the param — it is a property of what the preset's palette is *for*.

That is the same shape as the fold's edge: a behaviour with no universally right answer, where the
author knows which one their look wants. [ADR-0061](0061-kaleidoscope-edge-treatment-is-a-per-preset-choice.md)
resolved that one by making it a choice.

## Decision

**The palette coordinate's edge behaviour becomes a per-preset `[palette]` choice** — wrap (today's
behaviour, and the default, so every shipped preset is byte-identical) or clamp, applied to the
**final** coordinate at every LUT fetch site. A preset whose colour language is a *ramp from dark to
bright* declares clamp and stops having a wrap edge at all; a preset built on hue rotation keeps the
periodic wheel it needs.

**It is implemented as a scalar in the existing uniform, not as a second sampler.** Two samplers
would mean two bind-group layouts on a stage that already has an enumeration guard
([ADR-0058](0058-bind-group-layout-collisions-carry-evidence.md)), for a behaviour a `select` in the
shader expresses exactly. The clamp bound is **the texel-centre range, not `[0, 1]`** — with linear
filtering and a repeating sampler, a coordinate of exactly `0` or `1` still blends texel 255 into
texel 0, so clamping to `[0, 1]` would leave a narrow band of the wrap it exists to remove. Anything
built here needs that stated, and needs a test at both ends that would fail against a naive
`clamp(c, 0.0, 1.0)`.

**No plan yet, by the user's call.** The want is real but nobody is blocked: both shipped IFS presets
bind the route that works (`root_hue`, not `root_tint`), no shipped content is wrong, and the edge is
documented. This ADR exists so that the decision is not re-litigated from scratch the day someone
authors a dark-to-bright ramp and drives a positional coordinate across its floor.

## Consequences

**Positive**

- **A positional coordinate stops having a surprising edge**, without taking the periodic wheel away
  from the params that are genuinely cyclic. Both behaviours remain reachable, which is the only way
  a single shared coordinate can serve both.
- **The default is today's behaviour**, so the decision costs zero pixels and no golden re-bless when
  it lands. That is what makes it safe to defer: the deferral is not accruing a migration.
- **It puts the choice where the knowledge is.** Whether a palette is a wheel or a ramp is something
  the author decided when they wrote the stops; nothing in the engine can infer it.
- The precedent generalizes rather than being spent: this is the second edge in the engine to become
  a per-preset choice for the same reason, and the shape is now twice-used rather than one-off.

**Negative**

- **One more `[palette]` key**, on a table the content lane already reads as its authority, for a
  behaviour most presets will never set. Every param is a cost this project makes authors pay in
  attention; the mitigation is that the default means an author who does not care never meets it.
- **It does not make the coordinate's *composition* legible**, which is arguably the deeper problem:
  an author who stacks `hue_center`, `hue_spread`, `map_tint` and a root term is summing four things
  into one number with no readout of where it landed. This ADR treats the edge, not the arithmetic.
  A coordinate readout in `shot --report` would be the other half and is not proposed here.
- **A clamped ramp loses a real look.** The wrap is occasionally what an author wants on a positional
  coordinate too — a distance that cycles through the palette repeatedly is a legitimate banding
  effect. Declaring clamp gives that up for the whole preset, since there is one coordinate.
- **Deferring means the trap stays reachable.** The documentation is the only guard until this is
  built, and a doc line is what backlog 0078 and 0081 both proved rots or never lands.

## Alternatives considered

- **A — classify each param as cyclic or positional, and clamp the positional ones.** The obvious
  design, and it is **structurally impossible**: the contributors are summed into a single
  coordinate before the fetch, so there is nothing left to classify by the time the addressing
  applies. Clamping a *term* does not help either, since another term can carry the sum across zero.
  This is the alternative that made the per-preset answer the right one rather than a compromise.
- **B — clamp everywhere; drop the repeat.** One line, no new param. Rejected outright: it falsifies
  six documented behaviours that are *used* — `color_center`'s cyclic wrap, `color_span > 1`,
  `depth_hue`'s wrap bound, the banded palette's steps — and hue is genuinely an angle, so clamping
  it is not a conservative choice but a wrong one.
- **C — document the edge and change nothing.** The cheapest answer, and the honest observation is
  that the doc half **already landed** (Plan 0074's close, with the anchored-term property, the
  negative escape, the wrap and the fern's `−0.38` arithmetic) and the trap is still reachable. It
  remains the right *interim* state, which is why nothing is being built yet — but a rule an author
  must compute to avoid is not the same as a rule they cannot trip, and this project has three
  documented instances of a preset paying for a defect a header comment remembered.
- **D — bound `root_tint`'s own range so it cannot drive the sum negative.** Rejected as
  special-casing: the sum can cross zero from any contributor, `root_tint` is merely the first
  anchored term to ship, and clamping a lever's range to protect a downstream fetch hides the
  mechanism from the author who then cannot reason about the next one.
- **E — a second sampler with `ClampToEdge`, selected per preset.** Same user-visible behaviour as
  the decision. Rejected on cost: it adds a bind-group layout to a stage with an ADR-0058
  enumeration guard, for arithmetic a `select` in the shader already expresses. The
  texel-centre-clamp caveat above applies either way.
