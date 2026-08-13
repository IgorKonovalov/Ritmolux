# ADR-0106 — Two-tone graphics come from a multiply layer, not a composite redesign

> **Status:** accepted (2026-08-13, user approval)
> **Date:** 2026-08-13
> **Related plan(s):** [0091](../plans/0091-the-figure-fills-the-frame.md)

## Context

[design-backlog 0069](../design-backlog.md) says the engine cannot draw a two-tone object — a fill
with a contrasting outline — and gives a mechanism: *"Black adds zero, so a dark edge cannot exist
inside the composite, and the only dark-on-light route in the engine is the ink stage, which is
structurally two-poled (`mix(paper, ink, luminance)`) and therefore cannot hold three tones
either."* It prices the repair as reopening ADR-0018's composite and ADR-0056's alpha model plus an
ordering story the additive pipeline has never needed, and files itself **Low priority** explicitly
because of that cost.

**That entry was raised 2026-08-05, at Plan 0070's close. The layer system landed 2026-08-11**
(ADR-0090 / Plan 0076). The entry predates the capability by six days, and nothing has revisited it
since — including three closes that ran in between.

Two facts in the shipped code meet:

- `layer_blend.rs:52-54` — the four `over`-join blend modes operate in linear HDR light, and
  **"`multiply` clamps the layer operand so it strictly darkens"**, within the layer's
  premultiplied-alpha footprint (`coverage = layer.a * mix`).
- `fragment_field.rs:168` — a fullscreen field scene returns `vec4<f32>(col, params.d.y)`, so its
  alpha is `occlude`, **default 1, on every pixel including the black ones**. ADR-0056 states the
  invariant directly: *"A fullscreen field scene covering every pixel correctly emits 1."*

A field scene therefore has full coverage where it is dark, which is exactly the condition a
darkening blend needs and exactly the condition a *particle* scene fails — a particle's alpha is
its falloff (`vec4(color * g, g)`), so a black particle has no coverage and cannot darken anything.
The backlog entry's mechanism is right about particles and wrong about fields.

**Measured 2026-08-13**, on this box's hardware adapter at 640x360 after 60 frames. One preset, one
word changed, everything else held: a fullscreen field pinned flat by `color_span = 0` as the
chain, the same system sampling the whole gradient at `palette_steps = 5` as an `over` layer at
`mix = 1`, `bg_bright = 0` so the backdrop is not in the picture.

| layer `blend` | min luma | mean | max | pixels below luma 64 |
|---|---|---|---|---|
| `multiply` | **18.5** | 80.9 | 207.3 | **61.9 %** |
| `add` (control) | **181.6** | 207.7 | 233.1 | **0.0 %** |

The additive control cannot reach below luma 181.6 anywhere in the frame. The multiply frame
reaches 18.5, puts nearly two thirds of its pixels in the dark range, and carries the palette's red
as a distinct middle tone between the dark and pale ones — three tones in one frame, which is the
thing the entry says is impossible.

## Decision

**Two-tone and three-tone graphics are authored as a fullscreen field scene joined `multiply` over
a lighter ground, and we do not redesign the composite.** The capability shipped with ADR-0090; what
was missing was knowing it, so this is a documentation and correction task rather than an
engine one.

design-backlog 0069's fill-and-outline ask is **answered for field scenes**. Its entry is corrected
in place rather than archived, because **its other half is untouched**: multiply darkens in
proportion to coverage, and nothing in this engine still decides what is *in front* of what. A
shaped object that occludes another figure remains unbuilt, and that is what the entry keeps.

## Consequences

### Positive

- **The reference collage's dark-on-light floor and its red figure stop being a composite
  redesign** and become an authoring pattern, at the cost of documenting it.
- **ADR-0018 and ADR-0056 stay closed.** The alpha model that made this work is the one ADR-0056
  already chose, for a different reason — the coverage invariant it introduced to stop the backdrop
  being subtracted is the same invariant that makes a dark field darken.
- **The ink stage stops being the only dark-on-light route**, which matters because ink takes the
  whole frame: a purple sky and a red figure cannot coexist with it, and they can with this.

### Negative

- **It is scoped to field scenes and the boundary is invisible from the preset side.** A `swarm`
  or `emitter` in a multiply layer cannot darken, because its alpha *is* its brightness. Two routes
  to the same shape — the heart as a particle mark and the heart as a field — now have different
  colour capabilities for a reason a preset author cannot see in the parameter table. This is a real
  authoring trap and the docs have to name it, not merely describe the working case.
- **"Black" is dark grey.** The darkest reachable in the probe is luma 18.5, not 0. Something
  downstream lifts it — bloom and the tonemap are both live and were not separated — so a preset
  asking for a true black outline does not get one. The measurement is honest about the number;
  the mechanism is not yet established.
- **It costs the preset's only `[layer]` slot.** ADR-0090 caps a preset at one layer table, so a
  two-tone preset cannot also carry a second figure. Two-tone and counterpoint are mutually
  exclusive until that cap moves.
- **Whether multiply reaches the *backdrop* is unmeasured.** The probe ran at `bg_bright = 0`, so
  it establishes the layer-over-chain path only. `post.rs:33` says "the backdrop is **not** in the
  chain's input" — it is composited underneath — so the backdrop case is a genuinely separate
  question, and Plan 0091 opens by measuring it rather than assuming it.

### Neutral

- The measurement is a *property* under ADR-0071 in the direction that matters (the additive control
  cannot go dark; the multiply one does), and a *machine-named measurement* in its exact digits. The
  separation of 181.6 against 18.5 is far too large to be adapter drift, but the digits themselves
  name this box.

## Alternatives considered

### Alternative A — redesign the composite as backlog 0069 proposes

Reopen ADR-0018 and ADR-0056, add an ordering or sorting story, make objects opaque and occluding.
Rejected because the measurement shows the *stated ask* — a red fill with a contrasting outline —
is reachable without any of it. This alternative is not discarded so much as reduced: what survives
is the occlusion question alone, which is what backlog 0069 now keeps.

### Alternative B — the ink stage

`ink_*` already maps dark input to paper and bright to ink, and `attractor_valentine` ships a pink
heart on a white page through it. Rejected as the general answer because it is structurally
two-poled — `mix(paper, ink, luminance)` — so it cannot hold three tones, and because it is a
terminal full-frame remap: the reference collage's purple sky cannot survive a stage that discards
hue and repaints the whole frame between two poles.

### Alternative C — leave backlog 0069 as written and note the finding elsewhere

Rejected on this project's own rule: a live backlog entry whose premise is false is more dangerous
than a closed one, because it sends the next reader to do work that is already unnecessary — and
here the work in question is a composite redesign that the entry itself prices as the reason it is
Low priority. The correction goes in the entry.

## Notes

The probe preset and the luma statistics script are scratch artifacts, not committed. What is
reproducible from this ADR is the construction: a flat chain (`color_span = 0`), a banded field
layer at `mix = 1`, `bg_bright = 0`, and the same file rendered twice with `multiply` and `add`.

`overlay` was not measured. It branches per channel on the destination and is the mode most likely
to be useful for a *contrasting outline* specifically, since it darkens and lightens from one
operand — worth a look when someone authors the first two-tone preset.
