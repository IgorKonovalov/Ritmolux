# ADR-0138 — Limited ink is a supported palette class, defined at the draw seam

> **Status:** accepted 2026-08-28 (Plan 0123)
> **Date:** 2026-08-27
> **Related plan(s):** [0123](../plans/done/0123-a-gate-a-latch-and-an-ink.md)
> **Supplements:** [0056](0056-additive-scenes-emit-premultiplied-alpha.md), [0133](0133-the-band-contour-fires-where-the-ink-changes.md)

## Context

The mono cohort is built on a palette quantized into flat plateaus — black, white and one red — and
it works. It works on four of the engine's twelve systems, and the reason is the blend mode. Every
line and particle scene draws additively and overlaps itself, so white laid over red sums to pink
and the plateaus are gone before the frame is composited. The ink set survives only where colour is
resolved per-pixel each frame (`fragment_field`, `shape_field`, `reaction_diffusion`) or painted
opaque (`shape_collage`).

Nothing in the engine says this. There is no declared class, no invariant, and no statement anywhere
of what a scene owes a quantized palette — so the confinement is not a decision anyone took, it is
the residue of `ADDITIVE_LIGHT_SATURATING_COVERAGE` being the one seam every line scene draws
through. A content lane wanting a Maurer rose in black, white and red has no way to learn that it
cannot have one except by rendering it, and every new scene is built additive by default, so the
confined fraction grows on its own.

The mechanism is already half built and that changes what this costs. `LineRenderer` carries **two**
pipelines differing in exactly one field — the additive seam and premultiplied OVER — and
`draw_split` already composites a batch's second range with each segment's own coverage, built at
Plan 0100 Phase 4 so a MilkDrop waveform at `wave_a = 0.1` replaces a tenth of what is under it
rather than all of it. One shader serves both, the bind layout is the same, and the whole facility
has exactly one caller (`warp_mesh`) and no preset-reachable selector.

The honest complication: a whole-frame guarantee is falsified on arrival. Bloom, the tonemap, trails,
the kaleidoscope's resampling and `palette_contour` all introduce intermediate values, and
[backlog 0140](../design-backlog.md) measured the last of them — turning `palette_contour` on takes
`shape_contourmono` from **9 distinct colours to 684**. So "N inks means N colours in the frame" is
not a claim this engine can make.

## Decision

We will declare **limited ink a supported palette class**, and define its guarantee at the **scene's
draw seam** rather than over the finished frame:

> On a scene drawing through an opacity-preserving seam, with a fully quantized palette, the
> **scene's own output** contains only colours the palette names. Every later stage that introduces
> intermediate values is enumerated, and each one names the parameter that disables it.

Defining it at the seam is what makes it true and checkable. The seam is a property of a draw call
and is decided by the engine; the finished frame is the product of a post chain the preset composes,
and any guarantee over it would have to encode the post chain's exemption list as thresholds — which
is the thing that rots. An author who wants a strictly N-colour frame follows the enumerated list and
gets one; an author who wants bloom keeps bloom and knows exactly what they traded.

Two things follow immediately, and they are the whole of the first delivery:

- **The line family gets a preset-reachable blend selector**, using the OVER pipeline that already
  exists. Four systems — `parametric_curve`, `lsystem`, `star_pattern`, `spectrum` — move from
  "structurally closed to the class" to "in it, when the preset asks."
- **The intermediate-value stages get written down**, in `docs/preset-palettes.md`, as a complete
  enumeration with each one's off switch. That list is the class's real contract and the thing the
  content lane reads.

**The class convicts `palette_contour`, and that is a feature of the question.** ADR-0133 fixed which
edges the contour draws; backlog 0140 is about what it draws — a soft scalar darken toward black,
with no ink of its own. Under this class it is an enumerated intermediate-value stage whose off
switch is `palette_contour = 0`, which costs `shape_contourmono` the key line it currently earns.
Naming that is the point: the class does not repair the contour, it makes the trade legible and gives
0140 a contract to be repaired against.

**Particles and compute stay outside the class for now.** They are a different renderer with nothing
equivalent already sitting in it, and pretending otherwise would make the class a promise the engine
does not keep.

## Consequences

### Positive

- **A whole palette class stops being an emergent trick.** The mono cohort is currently built on a
  coincidence of four scenes; after this it is built on a stated property, and a fifth scene knows
  what it owes.
- **The first delivery is nearly free.** The OVER pipeline, its shader, its bind layout and its
  ordering semantics were built and tested at Plan 0100. What is missing is a selector and a
  parameter.
- **The enumeration is useful on its own**, independent of the class. "Which stages put intermediate
  values into my frame, and how do I turn each off" is a question the content lane asks constantly
  and currently answers by experiment.
- **It gives [backlog 0140](../design-backlog.md) a contract.** That entry has two candidate fixes
  and no way to choose between them; a class with a stated guarantee is the thing that decides which.

### Negative

- **`palette_contour` is convicted on delivery**, and one shipped preset (`shape_contourmono`) is
  where that lands. Nothing breaks, and the entry documents that the workaround looks good — but the
  class says out loud that a shipped preset is outside it, which is uncomfortable and correct.
- **Two systems are named as excluded.** Particles and compute are outside a class the engine now
  advertises, so the docs carry an asymmetry that will read as an omission until it is closed.
- **A per-scene blend selector is a new parameter on four scenes**, and blend mode is the kind of
  parameter that interacts with everything — glow, trails, the composite. A preset that flips it is
  a visually different preset, so every affected golden moves.
- **A guarantee at the seam is weaker than the one people will assume.** "Limited ink is supported"
  will be read as a frame-level promise, and the docs have to keep saying it is not, in the same
  place, every time.

### Neutral

- No change to the additive seam itself, to ADR-0056's premultiplied contract, or to any scene's
  default. A preset that does not ask for the new seam renders exactly as it does today.
- No new gate. Whether the class should eventually be enforced by a colour-counting test is left
  open by this ADR, and Alternative B says why it is not enforced now.

## Alternatives considered

### Alternative A — ship the blend selector, decide nothing

Make the OVER pipeline preset-reachable on the line family and stop. The cheapest useful thing, and
it unblocks the motivating look. Rejected because it answers the instance and not the question: every
future scene would still be built additive by default with nothing telling it what a quantized
palette needs, and the confined fraction would go on growing exactly as it has. The selector without
the class is a lever with no contract behind it; the class without the selector is a document. The
decision here is that the contract is the durable half and the selector is its first instalment.

### Alternative B — a frame-level invariant with a colour-counting gate

"A preset declaring N inks renders in exactly N colours", enforced by a test that counts distinct
colours in a capture. Rejected because it is false on delivery and the repair makes it worse. Bloom,
the tonemap, trails, kaleidoscope resampling and `palette_contour` each break it, so the gate would
have to carry an exemption list expressed as tolerances — a count "near enough" N — and a tolerance
on a colour count is a number with no mechanism behind it, which is exactly what
[ADR-0071](0071-a-numeric-test-contract-states-a-property-or-names-its-machine.md) exists to reject. The
seam-level guarantee is the largest true statement available, and 0140's own measurement (9 colours
to 684, from one parameter) is the evidence that the frame-level one is not.

### Alternative C — remove the additive default and make every scene opacity-preserving

The clean-sheet answer, and it would make the class universal. Rejected outright: additive light is
what the luminous worlds *are*, ADR-0056 built the premultiplied contract around it, and every scene
but `shape_collage` is designed against it. This would not be a palette decision, it would be a
different engine.

## Outcome (2026-08-28, Plan 0123 close)

Accepted as decided — the seam shipped, the enumeration shipped, and the class is reachable. Two
claims above are narrower in practice than they read, both measured at Plan 0123 Phases 8-9 and
recorded here rather than edited into the body.

- **"The scene's own output contains only colours the palette names" is true of the baked LUT, not
  of the hex an author writes.** `LUT_TEXTURE_FORMAT` is `Rgba8Unorm` and the entries are consumed
  as **linear** light (ADR-0021 Alt E's deferral), so a stop written as ordinary sRGB is lifted by
  the display encode: `#c81423` renders `#dd4c64`, and `collage_mono`'s `#b00808` arrives as
  `#d63131`. Writing the sRGB-to-linear value `#930204` instead renders `#c81622`, within 2/255 of
  the colour named — so **below the tonemap knee the shift is exactly correctable by the author**,
  which `docs/preset-palettes.md` does not yet say. Filed as design-backlog 0153.
- **"An author who wants a strictly N-colour frame follows the enumerated list and gets one" is a
  claim about plateaus, not about a colour count.** With every enumerated mixer off, `collage_mono`
  at 1280x720 comes back with three flat regions and **615 distinct colours** — the residue is
  ADR-0096's static display dither, one encoded level, which the enumeration correctly lists as the
  one leak with no off switch. Three plateaus is the deliverable and the count is not.
- The enumeration was **incomplete as drafted** and was completed during the sweep, as the phase
  required: the backdrop composite, the A/B palette crossfade, the duotone ink pass, an `over` layer
  join (all four `LayerBlend` variants mix; `join = "under"` is the off switch) and the internal post
  grid's linear resample are mixers this ADR does not name.
